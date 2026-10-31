//! The `--json` output contract: atip/1 L0 (0.33).
//!
//! In JSON mode stdout carries exactly one document and stderr nothing:
//!
//! - success `{"ok":true,"data":<report>,"meta":{…}}` ([`data`] and friends);
//! - failure `{"ok":false,"error":{…},"meta":{…}}`, plus `data` when the
//!   command keeps a partial report on exit 1
//!   ([`crate::cli_error::CliErrorExt::emit_with_data`]).
//!
//! `meta` holds only the members the contract defines and canon's own
//! `x-canon` ([`Meta`]): `schemaVersion`, `tool`, `toolVersion`, `contract`,
//! `command` (the resolved command id; absent when not even the first word
//! resolved), `output` (the report's name, when the command declares several),
//! `context` (the context options a command took: `repo`), `next` (runnable
//! steps with their `argv`), `warnings` (`{code, message}`), `coverage` and
//! `page` (`returned`, `total`, `nextCursor`). A member with no value is
//! absent, never `null` and never `[]`. Keys are camelCase; ledger records
//! embedded verbatim keep their own schema.
//!
//! What used to be flat `meta` keys is the report's: a write's `unchanged`
//! (a repeat found its work done), `waitingOn` and `nextOmitted` are members of
//! `data` ([`Report::STATE_IN_DATA`]). A report that is a stored record, or is
//! not an object, cannot take them, so they travel in `meta.x-canon`.
//!
//! A report is a [`Report`]: a struct with a derived JSON Schema, so an output
//! without a schema does not compile. In debug builds every document is
//! validated against the vendored contract schemas ([`atip::envelope_problems`])
//! and, when typed, the report's schema; a mismatch panics, so the whole test
//! suite is a check (T7).
//!
//! Human lines a command would print are said by [`say!`]: on stdout in human
//! mode, nowhere in JSON mode (the document carries the same account).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Map, Value};

pub use crate::reports::Report;

use crate::cli_error::{CliError, CliErrorExt as _, ErrorBody};
use crate::next::{After, NextEntry};

static UNCHANGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static BATCH_INPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A write that found its work already done with identical content (a repeat
/// of a `replay-safe` or `content-keyed` leaf, D10) says so with this before
/// it prints its report: the success envelope then carries `unchanged: true`
/// in `data` (in `meta.x-canon` for a report that cannot take it), and a batch
/// reports the element `unchanged`.
pub fn mark_unchanged() {
    UNCHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Records that the run's `--input` was a batch, so a failure without `data`
/// names the batch output's version (spec 6.11).
pub fn set_batch_input(batch: bool) {
    BATCH_INPUT.store(batch, std::sync::atomic::Ordering::Relaxed);
}

/// `meta.schemaVersion` before a command is resolved.
pub const DEFAULT_SCHEMA_VERSION: &str = "1.0";

/// `meta.tool`: the executable's name.
pub const TOOL: &str = "canon";

/// `meta.contract`: the contract the output follows.
pub const CONTRACT: &str = "atip/1";

/// `meta.context`: the context options a command took, with the value it used.
/// canon's only context option is `--repo`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// The repo root canon worked in, as a path relative to the current directory (`.` for it).
    pub repo: String,
}

/// One entry of `meta.warnings`: a condition that makes the result weaker than
/// it looks. The code is registered like an error code ([`crate::cli_error::all_codes`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, JsonSchema)]
pub struct Warning {
    pub code: String,
    pub message: String,
    /// The argument or field the warning is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl Warning {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.to_string(), message: message.into(), field: None }
    }
}

/// `meta.page`: where a page stands (spec 6.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub returned: usize,
    /// Present exactly when more items follow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Every record the command matched, across all pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
}

/// The `meta` member of every envelope. Every member but the first four is
/// absent when it has no value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// The command's id (`gate check`); flags never change it. Absent when not even the first word resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// `"<major>.<minor>"` of the report in `data`.
    pub schema_version: String,
    pub tool: &'static str,
    pub tool_version: String,
    pub contract: &'static str,
    /// The name of the output `data` follows, when the command declares more than one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Context>,
    /// The steps that follow, most useful first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Vec<NextEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<Warning>>,
    /// What a check or listing examined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<PageInfo>,
    /// canon's own metadata: what `data` cannot hold.
    #[serde(rename = "x-canon", skip_serializing_if = "Option::is_none")]
    pub x_canon: Option<Map<String, Value>>,
}

#[derive(Serialize)]
struct Success<'a, R: ?Sized> {
    ok: bool,
    data: &'a R,
    meta: Meta,
}

#[derive(Serialize)]
struct Failure<'a, R: ?Sized> {
    ok: bool,
    error: ErrorBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<&'a R>,
    meta: Meta,
}

/// A write's report with the members canon adds to it (see the module docs).
#[derive(Serialize)]
struct WithState<'a, R: ?Sized> {
    #[serde(flatten)]
    report: &'a R,
    #[serde(skip_serializing_if = "Option::is_none")]
    unchanged: Option<bool>,
    #[serde(rename = "waitingOn", skip_serializing_if = "Option::is_none")]
    waiting_on: Option<&'a BTreeMap<String, usize>>,
    #[serde(rename = "nextOmitted", skip_serializing_if = "Option::is_none")]
    next_omitted: Option<usize>,
}

/// A ledger record embedded verbatim in a report: a JSON object that keeps its
/// own snake_case schema (`crates/canon-model/schemas/<kind>.schema.json`),
/// possibly redacted or cut to the `--fields` asked for, so the report's schema
/// cannot pin its members.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct LedgerRecord(pub Value);

impl JsonSchema for LedgerRecord {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "LedgerRecord".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "description": "A ledger record, verbatim: it keeps its own schema (`crates/canon-model/schemas/<kind>.schema.json`), minus anything redacted or left out by `--fields`"
        })
    }
}

/// The type parameter of a failure that carries no `data`: only its absence is
/// printed, so it has no members and no schema.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct NoData;

impl JsonSchema for NoData {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "NoData".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        true.into()
    }
}

impl Report for NoData {
    const STATE_IN_DATA: bool = false;
}

/// A `--fields` projection of a typed report: some of its top-level members. It
/// serializes as the projected value and validates against the report's schema
/// without `required`.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct Projected<R>(pub Value, #[serde(skip)] std::marker::PhantomData<R>);

impl<R> Projected<R> {
    pub fn new(value: Value) -> Self {
        Self(value, std::marker::PhantomData)
    }
}

impl<R: JsonSchema> JsonSchema for Projected<R> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("Projected{}", R::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut schema = R::json_schema(generator);
        if let Some(object) = schema.as_object_mut() {
            object.remove("required");
            object.insert("description".to_string(), json!("A `--fields` projection: the requested top-level members of the full report"));
        }
        schema
    }
}

impl<R: Report> Report for Projected<R> {
    const SCHEMA_VERSION: &'static str = R::SCHEMA_VERSION;
}

/// `Some(Some(dir))`: the leaf took `--repo dir`; `Some(None)`: it takes `--repo` and was not given one;
/// `None` (never set, or [`set_repo_unused`]): the command names no repo, so `meta.context` is absent.
static REPO: OnceLock<Option<Option<PathBuf>>> = OnceLock::new();
static WARNINGS: std::sync::Mutex<Vec<Warning>> = std::sync::Mutex::new(Vec::new());
static FIRST_NEXT: std::sync::Mutex<Vec<crate::status::NextStep>> = std::sync::Mutex::new(Vec::new());
static INVOCATION: OnceLock<Vec<String>> = OnceLock::new();

/// Says a warning: on stderr in human mode; in JSON mode it waits for the
/// document and travels in `meta.warnings`. `code` is registered in
/// [`crate::cli_error::all_codes`]; one situation is one code, and the same
/// code with the same message is said once.
pub fn warn(code: &'static str, message: String) {
    warn_at(code, None, message);
}

/// [`warn`] naming the argument or field the warning is about.
pub fn warn_at(code: &'static str, field: Option<&str>, message: String) {
    debug_assert!(crate::cli_error::code_info(code).is_some(), "warning code {code} is not in cli_error::all_codes()");
    if atip::json_mode() {
        let warning = Warning { code: code.to_string(), message, field: field.map(str::to_string) };
        let mut warnings = WARNINGS.lock().expect("the warnings list is not poisoned");
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    } else {
        eprintln!("{message}");
    }
}

/// Puts `step` first in the `meta.next` of whatever envelope this run prints, a failure's included
/// (0.28 D9: `status`, `gate check` and `context` lead a 0.27-era repo to `canon migrate --json`).
/// An envelope without `next` gets one; one with the limit reached drops its last step into
/// `data.nextOmitted`. A plain `canon migrate` step already in the list is replaced by this one.
pub fn prepend_next(step: crate::status::NextStep) {
    FIRST_NEXT.lock().expect("the first-step list is not poisoned").push(step);
}

/// Remembers what the parse resolved about where the command works, so
/// `meta.context` can say which repo it worked in: `Some(--repo as given)` for
/// a leaf that takes `--repo`, `None` for one that takes no context option.
/// Called once by `main`, after the parse.
pub fn set_repo(repo: Option<PathBuf>) {
    let _ = REPO.set(Some(repo));
}

/// Records that the command takes no context option: `meta.context` is absent.
pub fn set_repo_unused() {
    let _ = REPO.set(None);
}

/// The argv the run was parsed from (after `--input` expansion): the words
/// after the program name. The next-page step repeats it with a new `--cursor`.
pub fn set_invocation(args: Vec<String>) {
    let _ = INVOCATION.set(args);
}

/// `command` as the schema id: `canon gate check`, `gate check` and `canon report check` are `gate check` and `report`.
pub fn command_id(command: &str) -> String {
    let mut words = command.split_whitespace().peekable();
    if words.peek() == Some(&"canon") {
        words.next();
    }
    words.take_while(|word| !word.starts_with('-')).collect::<Vec<_>>().join(" ")
}

/// `path` relative to the current directory, with `..` steps where it lies outside it; `.` for the directory itself.
pub fn relative_to_cwd(path: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    // The path's own components, absolute, with `.` and `..` folded away.
    let parts = |p: &Path| -> Vec<std::ffi::OsString> {
        let absolute = if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) };
        let mut parts: Vec<std::ffi::OsString> = Vec::new();
        for component in absolute.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    parts.pop();
                }
                other => parts.push(other.as_os_str().to_owned()),
            }
        }
        parts
    };
    let (target, base) = (parts(path), parts(&cwd));
    let shared = target.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut relative = PathBuf::new();
    for _ in shared..base.len() {
        relative.push("..");
    }
    relative.extend(&target[shared..]);
    if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        relative.to_string_lossy().replace('\\', "/")
    }
}

fn context() -> Option<Context> {
    let given = REPO.get()?.as_ref()?;
    let repo = crate::context::resolve_repo_root(given.as_deref().unwrap_or(Path::new(".")));
    Some(Context { repo: relative_to_cwd(&repo) })
}

/// Whether `command` (a canonical path) writes, per [`crate::schema::META`].
fn writes(command: &str) -> bool {
    crate::schema::META.iter().any(|row| row.path == command && row.effect != crate::schema::Effect::Read)
}

/// Whether the reports of `command` carry `unchanged`, `waitingOn` and `nextOmitted`
/// beside their own members: every write, and `status`, which names its steps.
pub fn carries_state(command: &str) -> bool {
    command == "status" || writes(command)
}

/// The members a write's report gains, as schema properties (they are optional).
pub fn state_properties() -> Map<String, Value> {
    let Value::Object(map) = json!({
        "unchanged": { "type": "boolean", "description": "true when a repeat found its work already done with identical content and wrote nothing; absent otherwise" },
        "waitingOn": { "type": "object", "additionalProperties": { "type": "integer", "minimum": 0 }, "description": "With a declared role: how many steps each other owner holds (`human` for the steps only the human takes)" },
        "nextOmitted": { "type": "integer", "minimum": 1, "description": "How many further `meta.next` steps were left out past the limit" }
    }) else {
        unreachable!("a JSON object literal")
    };
    map
}

/// `schema` with the members of [`state_properties`] added to its root.
pub fn with_state_properties(mut schema: Value) -> Value {
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for (name, property) in state_properties() {
            properties.entry(name).or_insert(property);
        }
    }
    schema
}

/// `base` with its minor raised by one: a report that gained members.
pub fn bumped(base: &str) -> String {
    match base.split_once('.') {
        Some((major, minor)) => match minor.parse::<u64>() {
            Ok(minor) => format!("{major}.{}", minor + 1),
            Err(_) => base.to_string(),
        },
        None => base.to_string(),
    }
}

/// The `meta.schemaVersion` of report `R` for `command`: the report's own
/// version, one minor higher where `command` adds the members of [`state_properties`].
pub fn schema_version_of<R: Report>(command: &str) -> String {
    if R::STATE_IN_DATA && carries_state(command) {
        bumped(R::SCHEMA_VERSION)
    } else {
        R::SCHEMA_VERSION.to_string()
    }
}

/// What a write adds to its report, read off the run.
#[derive(Debug, Default)]
struct State {
    unchanged: bool,
    waiting_on: Option<BTreeMap<String, usize>>,
    next_omitted: usize,
}

impl State {
    fn of(after: Option<&After>) -> Self {
        Self {
            unchanged: UNCHANGED.load(std::sync::atomic::Ordering::Relaxed),
            waiting_on: after.and_then(|after| after.waiting_on.clone()),
            next_omitted: after.map_or(0, |after| after.omitted),
        }
    }

    fn is_empty(&self) -> bool {
        !self.unchanged && self.waiting_on.is_none() && self.next_omitted == 0
    }

    fn into_x_canon(self, x_canon: &mut Map<String, Value>) {
        if self.unchanged {
            x_canon.insert("unchanged".to_string(), json!(true));
        }
        if let Some(waiting_on) = self.waiting_on {
            x_canon.insert("waitingOn".to_string(), json!(waiting_on));
        }
        if self.next_omitted > 0 {
            x_canon.insert("nextOmitted".to_string(), json!(self.next_omitted));
        }
    }
}

/// The extra `meta` members a call passes: the keys it may hold are
/// `warnings`, `coverage`, `page` and `x-canon`.
struct Extra {
    warnings: Vec<Warning>,
    coverage: Option<Value>,
    page: Option<PageInfo>,
    x_canon: Map<String, Value>,
}

fn split_extra(extra: Value) -> Extra {
    let mut out = Extra { warnings: Vec::new(), coverage: None, page: None, x_canon: Map::new() };
    let Value::Object(map) = extra else { return out };
    for (key, value) in map {
        match key.as_str() {
            "warnings" => out.warnings = serde_json::from_value(value).expect("`warnings` holds `{code, message}` objects"),
            "coverage" => out.coverage = Some(value),
            "page" => out.page = Some(PageInfo { returned: value["returned"].as_u64().map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX)), next_cursor: value["nextCursor"].as_str().map(str::to_string), total: value["total"].as_u64().map(|n| usize::try_from(n).unwrap_or(usize::MAX)) }),
            "x-canon" => {
                if let Value::Object(object) = value {
                    out.x_canon.extend(object);
                }
            }
            other => {
                debug_assert!(false, "`{other}` is not a meta member: use `x-canon` or put it in `data`");
                out.x_canon.insert(other.to_string(), value);
            }
        }
    }
    out
}

/// The `--cursor` step of a page: the same invocation with the next cursor.
fn next_page_step(cursor: &str) -> Option<NextEntry> {
    let invocation = INVOCATION.get()?;
    let mut argv: Vec<String> = Vec::with_capacity(invocation.len() + 2);
    let mut words = invocation.iter().peekable();
    let mut inserted = false;
    while let Some(word) = words.next() {
        if word == "--" && !inserted {
            argv.extend(["--cursor".to_string(), cursor.to_string()]);
            inserted = true;
        }
        if word == "--cursor" {
            words.next();
            continue;
        }
        if word.starts_with("--cursor=") {
            continue;
        }
        argv.push(word.clone());
    }
    if !inserted {
        argv.extend(["--cursor".to_string(), cursor.to_string()]);
    }
    NextEntry::runnable(argv, "the next page of this listing", None)
}

/// The `meta` for `command`: its steps, then `extra`'s members. `extra`'s
/// `warnings` and every warning said so far become `meta.warnings`. The second
/// value counts the steps a lead step ([`prepend_next`]) pushed past the limit.
fn build_meta(command: &str, schema_version: String, output: Option<String>, after: Option<&After>, extra: Value) -> (Meta, usize) {
    let command = Some(command_id(command)).filter(|id| !id.is_empty());
    let Extra { mut warnings, coverage, page, x_canon } = split_extra(extra);
    for said in std::mem::take(&mut *WARNINGS.lock().expect("the warnings list is not poisoned")) {
        if !warnings.contains(&said) {
            warnings.push(said);
        }
    }
    let mut next: Vec<NextEntry> = after.map(After::entries).unwrap_or_default();
    let mut pushed_out = 0;
    let first = std::mem::take(&mut *FIRST_NEXT.lock().expect("the first-step list is not poisoned"));
    if !first.is_empty() {
        let mut entries: Vec<NextEntry> = first.iter().filter_map(NextEntry::from_step).collect();
        let lead = entries.len();
        entries.extend(next.drain(..).filter(|entry| entry.command != "canon migrate"));
        let kept = crate::status::NEXT_LIMIT.max(lead);
        pushed_out = entries.len().saturating_sub(kept);
        entries.truncate(kept);
        next = entries;
    }
    if let Some(step) = page.as_ref().and_then(|page| page.next_cursor.as_deref()).and_then(next_page_step) {
        next.insert(0, step);
    }
    let meta = Meta {
        command,
        schema_version,
        tool: TOOL,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        contract: CONTRACT,
        output,
        context: context(),
        next: (!next.is_empty()).then_some(next),
        warnings: (!warnings.is_empty()).then_some(warnings),
        coverage,
        page,
        x_canon: (!x_canon.is_empty()).then_some(x_canon),
    };
    (meta, pushed_out)
}

fn success_text<R: Report>(command: &str, data: &R, after: Option<&After>, extra: Value) -> String {
    let id = command_id(command);
    let mut state = State::of(after);
    let output = output_of::<R>(&id);
    let (mut meta, pushed_out) = build_meta(command, schema_version_of::<R>(&id), output, after, extra);
    state.next_omitted += pushed_out;
    let in_data = R::STATE_IN_DATA && carries_state(&id);
    let text = if in_data && !state.is_empty() {
        let wrapped = WithState { report: data, unchanged: state.unchanged.then_some(true), waiting_on: state.waiting_on.as_ref(), next_omitted: (state.next_omitted > 0).then_some(state.next_omitted) };
        atip::line(&Success { ok: true, data: &wrapped, meta })
    } else {
        if !state.is_empty() {
            state.into_x_canon(meta.x_canon.get_or_insert_with(Map::new));
        }
        atip::line(&Success { ok: true, data, meta })
    };
    #[cfg(debug_assertions)]
    validate::<R>(&id, &text);
    text
}

/// The name of the output `data` follows: the batch output for a batch `--input` (whose elements the
/// binary carries as plain JSON), else the output of `R`; only when the command declares several.
fn output_of<R: Report>(command: &str) -> Option<String> {
    if BATCH_INPUT.load(std::sync::atomic::Ordering::Relaxed) {
        crate::reports::batch_output_name(command)
    } else {
        crate::reports::output_name::<R>(command)
    }
}

/// Prints the success envelope for `command` on stdout.
pub fn data<R: Report>(command: &str, data: &R) {
    atip::print_line(&success_text(command, data, None, Value::Null));
}

/// Prints a write's success envelope with the steps that follow it ([`crate::next`]).
pub fn data_with_next<R: Report>(command: &str, data: &R, after: &After) {
    data_with_next_and_meta(command, data, after, Value::Null);
}

/// [`data_with_next`] with extra `meta` members (a page, `coverage`, …).
pub fn data_with_next_and_meta<R: Report>(command: &str, data: &R, after: &After, extra_meta: Value) {
    atip::print_line(&success_text(command, data, Some(after), extra_meta));
}

/// [`data`] with extra `meta` members beside `command` (`page`, `coverage`, `warnings`).
pub fn data_with_meta<R: Report>(command: &str, data: &R, extra_meta: Value) {
    atip::print_line(&success_text(command, data, None, extra_meta));
}

/// The success envelope [`data`] prints, as a value: for a caller that answers
/// over a socket instead of stdout.
pub fn envelope<R: Report>(command: &str, data: &R) -> Value {
    serde_json::from_str(&success_text(command, data, None, Value::Null)).expect("the envelope is JSON")
}

/// The write envelope [`data_with_next_and_meta`] prints, as a value.
pub fn envelope_with_next<R: Report>(command: &str, data: &R, after: &After, extra_meta: Value) -> Value {
    serde_json::from_str(&success_text(command, data, Some(after), extra_meta)).expect("the envelope is JSON")
}

fn failure_text<R: Report>(command: &str, error: &CliError, data: Option<&R>, extra_meta: Value) -> String {
    let id = command_id(command);
    let (schema_version, output) = match data {
        Some(_) => (schema_version_of::<R>(&id), output_of::<R>(&id)),
        None => (crate::reports::declared_version(&id, BATCH_INPUT.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or_else(|| DEFAULT_SCHEMA_VERSION.to_string()), None),
    };
    let document = Failure { ok: false, error: error.body(), data, meta: build_meta(command, schema_version, output, None, extra_meta).0 };
    let text = atip::line(&document);
    #[cfg(debug_assertions)]
    validate::<R>(&id, &text);
    text
}

/// Prints the failure envelope on stdout (nothing on stderr), with `data` when
/// the command keeps a partial report.
pub fn print_failure<R: Report>(command: &str, error: &CliError, data: Option<&R>) {
    print_failure_with_meta(command, error, data, Value::Null);
}

/// [`print_failure`] with extra `meta` members beside `command` (`coverage`, …).
pub fn print_failure_with_meta<R: Report>(command: &str, error: &CliError, data: Option<&R>, extra_meta: Value) {
    atip::print_line(&failure_text(command, error, data, extra_meta));
}

/// The failure envelope as a value.
pub fn failure_value<R: Report>(command: &str, error: &CliError, data: Option<&R>) -> Value {
    serde_json::from_str(&failure_text(command, error, data, Value::Null)).expect("the envelope is JSON")
}

/// The `meta` member a command's warnings travel in: `{ "warnings":
/// [{code, message}, …] }`, or null when there is none. A warning is also said
/// on stderr in human mode by the command that raises it.
pub fn warnings_meta(warnings: &[Warning]) -> Value {
    if warnings.is_empty() {
        Value::Null
    } else {
        json!({ "warnings": warnings })
    }
}

/// Splits a payload that carries its own page `meta` ([`crate::paging`]'s
/// `page`) into `(data, meta)`, so the page state lands in `meta` beside
/// `command` instead of inside `data`.
pub fn split_meta(mut payload: Value) -> (Value, Value) {
    let meta = payload.as_object_mut().and_then(|object| object.remove("meta")).unwrap_or(Value::Null);
    (payload, meta)
}

/// Debug builds check every document against the contract's envelope schema
/// and the report's own schema (T7). A mismatch is a bug in canon, not in the caller.
#[cfg(debug_assertions)]
fn validate<R: Report>(command: &str, text: &str) {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    use jsonschema::Validator;

    type ReportValidators = Mutex<HashMap<(String, &'static str), Option<std::sync::Arc<Validator>>>>;
    static REPORTS: LazyLock<ReportValidators> = LazyLock::new(|| Mutex::new(HashMap::new()));

    let document: Value = serde_json::from_str(text).expect("a document is JSON");
    witness::<R>(&document);
    let ok = document["ok"].as_bool().unwrap_or(false);
    let problems = atip::envelope_problems(&document);
    assert!(problems.is_empty(), "JSON document breaks the atip/1 envelope schema: {problems:?}\n{text}");
    let registered = |code: &str| crate::cli_error::code_info(code).is_some();
    if !ok {
        let code = document["error"]["code"].as_str().unwrap_or_default();
        assert!(registered(code), "error code {code} is not in cli_error::all_codes()");
    }
    for warning in document["meta"]["warnings"].as_array().into_iter().flatten() {
        let code = warning["code"].as_str().unwrap_or_default();
        assert!(registered(code), "warning code {code} is not in cli_error::all_codes()");
    }
    let Some(data) = document.get("data") else { return };
    let validator = REPORTS
        .lock()
        .expect("the validator cache is not poisoned")
        .entry((command.to_string(), std::any::type_name::<R>()))
        .or_insert_with(|| {
            let mut schema = crate::reports::schema_of::<R>();
            if R::STATE_IN_DATA && carries_state(command) {
                schema = with_state_properties(schema);
            }
            (schema != Value::Bool(true)).then(|| std::sync::Arc::new(jsonschema::validator_for(&schema).unwrap_or_else(|e| panic!("the schema of {} does not compile: {e}", std::any::type_name::<R>()))))
        })
        .clone();
    if let Some(validator) = validator {
        let problems: Vec<String> = validator.iter_errors(data).map(|e| format!("{} at {}", e, e.instance_path())).collect();
        assert!(problems.is_empty(), "`data` breaks the schema of {}: {problems:?}\n{text}", std::any::type_name::<R>());
    }
}

/// T9: with `CANON_WITNESS_DIR` set, a debug build appends one line per document it
/// prints, `{"command", "ok", "type"}` (`type` is the report's Rust type), to
/// `<dir>/<pid>.ndjson`. `tests/grammar_t9_witness.rs` holds the lines of a whole
/// suite run against the outputs the spec declares.
#[cfg(debug_assertions)]
fn witness<R: Report>(document: &Value) {
    use std::io::Write;

    let Some(dir) = std::env::var_os("CANON_WITNESS_DIR") else { return };
    let line = json!({ "command": document["meta"]["command"], "ok": document["ok"], "type": std::any::type_name::<R>() });
    let path = Path::new(&dir).join(format!("{}.ndjson", std::process::id()));
    let written = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut file| writeln!(file, "{line}"));
    assert!(written.is_ok(), "CANON_WITNESS_DIR is set and {} cannot be written", path.display());
}

/// A command's narration: each line is said (stdout in human mode) and kept,
/// so the JSON result can carry the same account.
#[derive(Debug)]
pub struct Log {
    json: bool,
    lines: Vec<String>,
}

impl Log {
    pub fn new(json: bool) -> Self {
        Self { json, lines: Vec::new() }
    }

    pub fn line(&mut self, line: impl Into<String>) {
        let line = line.into();
        crate::say!(self.json, "{line}");
        self.lines.push(line);
    }

    pub fn into_lines(self) -> Vec<String> {
        self.lines
    }
}

/// `println!` in human mode, nothing in JSON mode (stdout carries the one
/// document, stderr nothing): `say!(json, "…", args)`.
#[macro_export]
macro_rules! say {
    ($json:expr, $($arg:tt)*) => {
        if !$json { println!($($arg)*); }
    };
}

/// `print!` in human mode, nothing in JSON mode: `say_raw!(json, "…", args)`.
#[macro_export]
macro_rules! say_raw {
    ($json:expr, $($arg:tt)*) => {
        if !$json { print!($($arg)*); }
    };
}

/// `eprintln!` in human mode, nothing in JSON mode: `say_err!(json, "…", args)`.
#[macro_export]
macro_rules! say_err {
    ($json:expr, $($arg:tt)*) => {
        if !$json { eprintln!($($arg)*); }
    };
}

/// Narration for a human: `eprintln!` unless the command runs in JSON mode
/// ([`atip::json_mode`]), where stderr stays empty and the document carries the result.
#[macro_export]
macro_rules! human_err {
    ($($arg:tt)*) => {
        if !::atip::json_mode() { eprintln!($($arg)*); }
    };
}

/// A warning: `eprintln!` in human mode, an entry of `meta.warnings` in JSON mode.
/// The first argument is the registered warning code ([`crate::cli_error::all_codes`]);
/// the rest is the message, as for `format!`.
#[macro_export]
macro_rules! warn_err {
    ($code:expr, $($arg:tt)*) => {
        $crate::json_out::warn($code, format!($($arg)*))
    };
}
