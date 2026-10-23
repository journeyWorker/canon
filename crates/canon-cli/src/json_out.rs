//! The `--json` output contract (0.20 D2, 0.28 D3).
//!
//! In JSON mode stdout carries exactly one document and stderr nothing:
//!
//! - success `{"ok":true,"data":<report>,"meta":{…}}` ([`data`] and friends);
//! - failure `{"ok":false,"error":{…},"meta":{…}}`, plus `data` when the
//!   command keeps a partial report on exit 1
//!   ([`crate::cli_error::CliError::emit_with_data`]).
//!
//! `meta` holds `command` (the schema id; flags never change it),
//! `schemaVersion`, `canonVersion`, `context` (role, actor, repo relative to
//! the current directory), `next` (each step with its `argv`), `warnings`,
//! paging, and the few keys a command adds (`unchanged`, `provenance`).
//! Keys are camelCase; ledger records embedded verbatim keep their own schema.
//!
//! A report is a [`Report`]: a struct with a derived JSON Schema, so an output
//! without a schema does not compile. In debug builds every document is
//! validated against the envelope schema and, when typed, the report's schema;
//! a mismatch panics, so the whole test suite is a check (T7).
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

use crate::cli_error::{CliError, ErrorBody};
use crate::next::{After, NextEntry};

/// `meta.schemaVersion` of a document that carries no typed report (a failure without `data`).
pub const DEFAULT_SCHEMA_VERSION: &str = "1.0";

/// `meta.context`: what canon resolved about where and as whom it ran.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// The declared role (`--as` or `CANON_ROLE`); `null` when none.
    pub role: Option<String>,
    /// The declared actor (`CANON_ACTOR`); `null` when none.
    pub actor: Option<String>,
    /// The repo root canon worked in, as a path relative to the current directory (`.` for it).
    pub repo: String,
}

/// The `meta` member of every envelope.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    /// The command's schema id (`gate check`); flags never change it.
    pub command: String,
    /// `"<major>.<minor>"` of the report in `data` (`"1.0"` in 0.28).
    pub schema_version: String,
    pub canon_version: String,
    pub context: Context,
    /// The steps that follow, most useful first; present on a command that writes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Vec<NextEntry>>,
    /// How many further steps were left out past the limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_omitted: Option<usize>,
    /// With a declared role: how many steps each other owner holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<BTreeMap<String, usize>>,
    /// Conditions that make the result weaker than it looks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
    /// Command-specific keys: paging (`returned`, `total`, `hasMore`, `nextCursor`), `unchanged`, `provenance`.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `{ok: true, data, meta}`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SuccessEnvelope {
    /// Always `true`.
    pub ok: bool,
    /// The command's report, named by `meta.command` in the spec's `outputs`. A ledger record inside it keeps the record's own schema (camelCase is the report's, not the record's).
    pub data: Value,
    pub meta: Meta,
}

/// `{ok: false, error, meta}` plus `data` when the command keeps a partial report on exit 1.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FailureEnvelope {
    /// Always `false`.
    pub ok: bool,
    pub error: ErrorBody,
    /// The partial report (gate violations, a blocked task, a refused promotion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    pub meta: Meta,
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

/// A payload with no schema yet. TEMPORARY: the slices that type the
/// `json!`-built payloads (plan S3a–e) remove it; nothing new may use it.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct Untyped(pub Value);

impl JsonSchema for Untyped {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Untyped".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        true.into()
    }
}

impl Report for Untyped {}

/// TEMPORARY entry point for a `json!`-built payload; see [`Untyped`].
pub fn untyped(value: Value) -> Untyped {
    Untyped(value)
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

impl<R: JsonSchema + serde::Serialize> Report for Projected<R> {}

static REPO: OnceLock<PathBuf> = OnceLock::new();
static JSON_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static WARNINGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Marks the process as running in JSON mode: stderr stays empty ([`human_err!`], [`warn_err!`]).
pub fn set_json_mode(json: bool) {
    JSON_MODE.store(json, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the command runs with `--json`.
pub fn json_mode() -> bool {
    JSON_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Says a warning: on stderr in human mode; in JSON mode it waits for the
/// document and travels in `meta.warnings`.
pub fn warn(message: String) {
    if json_mode() {
        let mut warnings = WARNINGS.lock().expect("the warnings list is not poisoned");
        if !warnings.contains(&message) {
            warnings.push(message);
        }
    } else {
        eprintln!("{message}");
    }
}

/// Remembers the `--repo` the command was given, so `meta.context.repo` can say
/// which repo it worked in. Called once by `main`, after the parse.
pub fn set_repo(repo: PathBuf) {
    let _ = REPO.set(repo);
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

fn context() -> Context {
    let repo = crate::context::resolve_repo_root(REPO.get().map(PathBuf::as_path).unwrap_or(Path::new(".")));
    Context {
        role: crate::role::Role::current(None).map(|role| role.as_str().to_string()),
        actor: std::env::var("CANON_ACTOR").ok().map(|actor| actor.trim().to_string()).filter(|actor| !actor.is_empty()),
        repo: relative_to_cwd(&repo),
    }
}

/// Whether `command` (a canonical path) writes, per [`crate::schema::META`].
fn writes(command: &str) -> bool {
    crate::schema::META.iter().any(|row| row.path == command && row.effect != crate::schema::Effect::Read)
}

/// The `meta` for `command`: its steps (a command that writes always carries
/// `next`, empty when it has no next step), then `extra`'s keys. `extra`'s
/// `warnings` become `meta.warnings`.
fn build_meta(command: &str, schema_version: &str, after: Option<&After>, extra: Value) -> Meta {
    let command = match command_id(command) {
        id if id.is_empty() => "canon".to_string(),
        id => id,
    };
    let mut extra = match extra {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let mut warnings = extra.remove("warnings").and_then(|value| serde_json::from_value::<Vec<String>>(value).ok()).unwrap_or_default();
    for said in std::mem::take(&mut *WARNINGS.lock().expect("the warnings list is not poisoned")) {
        if !warnings.contains(&said) {
            warnings.push(said);
        }
    }
    let warnings = (!warnings.is_empty()).then_some(warnings);
    let (next, next_omitted, waiting_on) = match after {
        Some(after) => (Some(after.entries()), (after.omitted > 0).then_some(after.omitted), after.waiting_on.clone()),
        None => (writes(&command).then(Vec::new), None, None),
    };
    Meta {
        command,
        schema_version: schema_version.to_string(),
        canon_version: env!("CARGO_PKG_VERSION").to_string(),
        context: context(),
        next,
        next_omitted,
        waiting_on,
        warnings,
        extra,
    }
}

fn success_text<R: Report>(command: &str, data: &R, after: Option<&After>, extra: Value) -> String {
    let document = Success { ok: true, data, meta: build_meta(command, R::SCHEMA_VERSION, after, extra) };
    let text = serde_json::to_string(&document).expect("a report serializes");
    #[cfg(debug_assertions)]
    validate::<R>(&text);
    text
}

/// Prints the success envelope for `command` on stdout.
pub fn data<R: Report>(command: &str, data: &R) {
    println!("{}", success_text(command, data, None, Value::Null));
}

/// Prints a write's success envelope with the steps that follow it ([`crate::next`]).
pub fn data_with_next<R: Report>(command: &str, data: &R, after: &After) {
    data_with_next_and_meta(command, data, after, Value::Null);
}

/// [`data_with_next`] with extra `meta` keys (a page's `nextCursor`, …).
pub fn data_with_next_and_meta<R: Report>(command: &str, data: &R, after: &After, extra_meta: Value) {
    println!("{}", success_text(command, data, Some(after), extra_meta));
}

/// [`data`] with extra `meta` keys beside `command` (`unchanged`, …).
pub fn data_with_meta<R: Report>(command: &str, data: &R, extra_meta: Value) {
    println!("{}", success_text(command, data, None, extra_meta));
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
    let schema_version = if data.is_some() { R::SCHEMA_VERSION } else { DEFAULT_SCHEMA_VERSION };
    let document = Failure { ok: false, error: error.body(), data, meta: build_meta(command, schema_version, None, extra_meta) };
    let text = serde_json::to_string(&document).expect("an error serializes");
    #[cfg(debug_assertions)]
    validate::<R>(&text);
    text
}

/// Prints the failure envelope on stdout (nothing on stderr), with `data` when
/// the command keeps a partial report.
pub fn print_failure<R: Report>(command: &str, error: &CliError, data: Option<&R>) {
    print_failure_with_meta(command, error, data, Value::Null);
}

/// [`print_failure`] with extra `meta` keys beside `command` (`coverage`, …).
pub fn print_failure_with_meta<R: Report>(command: &str, error: &CliError, data: Option<&R>, extra_meta: Value) {
    println!("{}", failure_text(command, error, data, extra_meta));
}

/// The failure envelope as a value.
pub fn failure_value<R: Report>(command: &str, error: &CliError, data: Option<&R>) -> Value {
    serde_json::from_str(&failure_text(command, error, data, Value::Null)).expect("the envelope is JSON")
}

/// The `meta` member a command's warnings travel in (0.25): `{ "warnings":
/// [<line>, …] }`, or null when there is none. A warning is also said on
/// stderr in human mode by the command that raises it.
pub fn warnings_meta(warnings: &[String]) -> Value {
    if warnings.is_empty() {
        Value::Null
    } else {
        json!({ "warnings": warnings })
    }
}

/// Splits a payload that carries its own page `meta` ([`crate::paging`]'s
/// `total`, `returned`, `nextCursor`) into `(data, meta)`, so the page
/// state lands in `meta` beside `command` instead of inside `data`.
pub fn split_meta(mut payload: Value) -> (Value, Value) {
    let meta = payload.as_object_mut().and_then(|object| object.remove("meta")).unwrap_or(Value::Null);
    (payload, meta)
}

/// Debug builds check every document against the envelope schema and the
/// report's own schema (T7). A mismatch is a bug in canon, not in the caller.
#[cfg(debug_assertions)]
fn validate<R: Report>(text: &str) {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    use jsonschema::Validator;

    static ENVELOPES: LazyLock<(Validator, Validator)> = LazyLock::new(|| {
        let schemas = crate::reports::envelope_schemas();
        let compile = |key: &str| jsonschema::validator_for(&schemas[key]).unwrap_or_else(|e| panic!("the {key} envelope schema does not compile: {e}"));
        (compile("success"), compile("failure"))
    });
    static REPORTS: LazyLock<Mutex<HashMap<&'static str, Option<std::sync::Arc<Validator>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

    let document: Value = serde_json::from_str(text).expect("a document is JSON");
    let ok = document["ok"].as_bool().unwrap_or(false);
    let envelope = if ok { &ENVELOPES.0 } else { &ENVELOPES.1 };
    let problems: Vec<String> = envelope.iter_errors(&document).map(|e| format!("{} at {}", e, e.instance_path())).collect();
    assert!(problems.is_empty(), "JSON document breaks the {} envelope schema: {problems:?}\n{text}", if ok { "success" } else { "failure" });
    let Some(data) = document.get("data") else { return };
    let validator = REPORTS
        .lock()
        .expect("the validator cache is not poisoned")
        .entry(std::any::type_name::<R>())
        .or_insert_with(|| {
            let schema = crate::reports::schema_of::<R>();
            (schema != Value::Bool(true)).then(|| std::sync::Arc::new(jsonschema::validator_for(&schema).unwrap_or_else(|e| panic!("the schema of {} does not compile: {e}", std::any::type_name::<R>()))))
        })
        .clone();
    if let Some(validator) = validator {
        let problems: Vec<String> = validator.iter_errors(data).map(|e| format!("{} at {}", e, e.instance_path())).collect();
        assert!(problems.is_empty(), "`data` breaks the schema of {}: {problems:?}\n{text}", std::any::type_name::<R>());
    }
}

/// A command's narration: each line is said (stdout in human mode) and kept,
/// so the JSON result can carry the same account.
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
/// ([`json_mode`]), where stderr stays empty and the document carries the result.
#[macro_export]
macro_rules! human_err {
    ($($arg:tt)*) => {
        if !$crate::json_out::json_mode() { eprintln!($($arg)*); }
    };
}

/// A warning: `eprintln!` in human mode, an entry of `meta.warnings` in JSON mode.
#[macro_export]
macro_rules! warn_err {
    ($($arg:tt)*) => {
        $crate::json_out::warn(format!($($arg)*))
    };
}
