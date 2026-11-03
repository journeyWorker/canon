//! The tool's fixture file (approaches/checker.md 5), the L0 and L1 keys of it.
//!
//! A fixture is a JSON object. At L0 the checker reads `contractVersion`, `setup`, `env`, `timeoutSeconds`,
//! `prepare` (programs run before `setup`), `probe`, `globalOptions`, `registry` (a list, or `"from-schema"`), `warnings` and, in `commands`, the invocations `ok`, `refused`,
//! `paged` and `missingValue`. At L1 it also reads, in `commands`, `fields` (selections) and `domainRejects` (values outside a
//! pattern or a structural domain, by argument). At L2 it reads, in `commands`, `batchFailsAt`, `batchFailsFirst`, `preview`,
//! `async` and `allowedEffects`, and `search`; a member this checker does not know is ignored when present.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::claim::{Claim, MINORS};

/// The default time limit of one invocation.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest time limit a fixture may ask for.
const MAX_TIMEOUT_SECONDS: f64 = 3600.0;

/// One row of the tool's error registry (spec 7.3), as the fixture lists it: the codes and reasons by name, no prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryRow {
    pub code: String,
    pub exit: i64,
    pub retryable: bool,
    /// The names of the reasons registered under the code; `None` when the fixture does not list them.
    pub reasons: Option<Vec<String>>,
}

/// The invocations of one command: argv arrays, each run in its own copy of the state, and the L1 lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandEntry {
    /// Invocations that succeed; the checker reads their envelopes (`meta.next`, `meta.page`) and builds its probes from them.
    pub ok: Vec<Vec<String>>,
    /// Invocations that are refused.
    pub refused: Vec<Vec<String>>,
    /// Invocations of the command with one required value left out (`L0.no-prompt`).
    pub missing_value: Vec<Vec<String>>,
    /// `--fields` selections: each a list of field paths (`L1.fields`).
    pub fields: Vec<Vec<String>>,
    /// Invocations of a paging command that return `meta.page.nextCursor` in the fixture state (`L0.page`, `L1.paging`).
    pub paged: Vec<Vec<String>>,
    /// Values outside a `pattern` or a structural domain, by argument name: the fixture author knows them, the checker cannot generate them (`L1.domains`, `L1.patterns`).
    pub domain_rejects: BTreeMap<String, Vec<String>>,
    /// Batches whose item `index` fails during execution: the documents and the index of the failing one (`L2.batch-incomplete`).
    pub batch_fails_at: Vec<BatchCase>,
    /// Batches whose first item fails during execution (`L2.batch-incomplete`).
    pub batch_fails_first: Vec<BatchCase>,
    /// Invocations of a destructive command that print its plan (`L2.destructive-preview` and the rows after it).
    pub preview: Vec<Vec<String>>,
    /// The async start of the command and the controllable worker (`L2.async`, `L2.operation-key`).
    pub async_entry: Option<AsyncEntry>,
    /// Glob patterns of the paths a successful `read` command of the fixture may change (`L2.effects`).
    pub allowed_effects: Vec<String>,
}

/// The `async` entry of a command (approaches/checker.md 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncEntry {
    /// The invocation that starts the work.
    pub start: Vec<String>,
    /// The environment variable the tool's test build reads to pick the outcome of its worker.
    pub worker: String,
    /// The outcomes the worker is driven to, of `done`, `failed` and `timeout`.
    pub outcomes: Vec<String>,
}

/// One question of the fixture's `search` and the command it expects among the first five entries (`L2.search`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub query: String,
    pub expect: String,
}

/// One batch of `--input` documents that stops at its item `index` (approaches/checker.md 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCase {
    pub input: Vec<Value>,
    pub index: usize,
}

/// What the checker reads from a fixture.
#[derive(Debug, Clone, Default)]
pub struct Fixture {
    /// The minor the tool claims; `None` when the fixture does not say (1.0).
    pub contract_version: Option<Claim>,
    /// Argv arrays run once, in order, to build the template directory.
    pub setup: Vec<Vec<String>>,
    /// The fixture's own environment variables.
    pub env: BTreeMap<String, String>,
    /// Programs the checker runs once in the template directory before `setup`, with `env` applied: environment preparation (`git init`, an empty commit) the tool cannot do itself. Argv arrays of any program, no shell; the tool's own state is `setup`'s.
    pub prepare: Vec<Vec<String>>,
    pub timeout: Duration,
    /// The id of the command that runs without state, split into words.
    pub probe: Option<Vec<String>>,
    /// Tool-wide options with a value, each an argv fragment such as `["--repo", "."]` (spec 9.13).
    pub global_options: Option<Vec<Vec<String>>>,
    /// The tool's error registry; `None` when the fixture gives none.
    pub registry: Option<Vec<RegistryRow>>,
    /// `"registry": "from-schema"`: the registry and the warning codes are the tool's own `schema --errors` and `--warnings` answers, and nothing is compared with the fixture (approaches/checker.md 5, spec 17.3).
    pub registry_from_schema: bool,
    /// The tool's warning codes; `None` when the fixture gives none.
    pub warnings: Option<Vec<String>>,
    /// The L0 invocations by command id; `None` when the fixture has no `commands`.
    pub commands: Option<BTreeMap<String, CommandEntry>>,
    /// Commands that do not end on their own (`notRun`: a server that serves until it is signalled). The checker runs each once, up to its first JSON
    /// document, and ends it by a signal; its other runs that would succeed are skipped and reported. From L1 each is a command of the tool's documents
    /// with an `effectNote`.
    pub not_run: Vec<String>,
    /// The questions `schema --search` is asked (`L2.search`); empty when the fixture has none.
    pub search: Vec<SearchQuery>,
}

impl Fixture {
    /// A fixture with nothing in it but the default time limit.
    pub fn empty() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            ..Self::default()
        }
    }

    /// The probe's id: its words joined by one space, as `meta.command` spells it.
    pub fn probe_id(&self) -> Option<String> {
        self.probe.as_ref().map(|words| words.join(" "))
    }

    /// The registry row of `code`, when the fixture has a registry and the code is in it.
    pub fn row(&self, code: &str) -> Option<&RegistryRow> {
        self.registry.as_ref()?.iter().find(|row| row.code == code)
    }

    /// The claim: the fixture's `contractVersion`, or 1.0.
    pub fn claim(&self) -> Claim {
        self.contract_version.unwrap_or(Claim::V1_0)
    }

    /// The first word of the first command id of two or more words: a group of the tool.
    pub fn group(&self) -> Option<String> {
        self.commands
            .as_ref()?
            .keys()
            .find_map(|id| id.split_once(' ').map(|(group, _)| group.to_string()))
    }

    /// Every `missingValue` invocation with the id of the command it belongs to.
    pub fn missing_values(&self) -> Vec<(&str, &Vec<String>)> {
        self.commands
            .iter()
            .flatten()
            .flat_map(|(id, entry)| entry.missing_value.iter().map(move |argv| (id.as_str(), argv)))
            .collect()
    }
}

/// Why a fixture could not be loaded: the `FIXTURE_INVALID` reason (spec 7.6) and one sentence naming what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub reason: &'static str,
    pub message: String,
}

/// Reads and validates the fixture at `path`.
pub fn load(path: &Path) -> Result<Fixture, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|error| LoadError {
        reason: "UNREADABLE",
        message: format!("cannot read {}: {error}", path.display()),
    })?;
    let document: Value = serde_json::from_str(&text).map_err(|error| LoadError {
        reason: "NOT_JSON",
        message: format!("{} is not JSON: {error}", path.display()),
    })?;
    parse(&document).map_err(|message| LoadError {
        reason: "WRONG_SHAPE",
        message,
    })
}

/// Validates a parsed fixture document.
pub fn parse(document: &Value) -> Result<Fixture, String> {
    let Some(object) = document.as_object() else {
        return Err("the fixture must be a JSON object".to_string());
    };
    let commands = object.get("commands").map(commands).transpose()?;
    let probe_words = object.get("probe").map(probe).transpose()?;
    let not_run = object.get("notRun").map(|value| not_run(value, probe_words.as_deref())).transpose()?.unwrap_or_default();
    Ok(Fixture {
        contract_version: object
            .get("contractVersion")
            .map(contract_version)
            .transpose()?,
        setup: object
            .get("setup")
            .map(setup)
            .transpose()?
            .unwrap_or_default(),
        env: object.get("env").map(env).transpose()?.unwrap_or_default(),
        prepare: object
            .get("prepare")
            .map(|value| argv_list(value, "prepare"))
            .transpose()?
            .unwrap_or_default(),
        timeout: object
            .get("timeoutSeconds")
            .map(timeout)
            .transpose()?
            .unwrap_or(DEFAULT_TIMEOUT),
        probe: probe_words,
        global_options: object
            .get("globalOptions")
            .map(global_options)
            .transpose()?,
        registry: match object.get("registry") {
            Some(Value::String(text)) if text == "from-schema" => None,
            other => other.map(registry).transpose()?,
        },
        registry_from_schema: matches!(object.get("registry"), Some(Value::String(text)) if text == "from-schema"),
        warnings: match (object.get("registry"), object.get("warnings")) {
            (Some(Value::String(_)), Some(_)) => {
                return Err("`warnings` must be left out when `registry` is \"from-schema\": the warning codes come from the tool too".to_string())
            }
            (_, warnings_value) => warnings_value.map(warnings).transpose()?,
        },
        commands,
        not_run,
        search: object.get("search").map(search).transpose()?.unwrap_or_default(),
    })
}

fn contract_version(value: &Value) -> Result<Claim, String> {
    value
        .as_str()
        .and_then(Claim::parse)
        .ok_or_else(|| {
            format!(
                "`contractVersion` must be one of the minors this checker knows: {}",
                MINORS.join(", ")
            )
        })
}

/// `notRun`: an array of command ids, each once, none of them the `probe` (approaches/checker.md 5). A command may be in `commands` too: its first `ok`
/// invocation is the one run, and the rest of its entries are skipped.
fn not_run(value: &Value, probe: Option<&[String]>) -> Result<Vec<String>, String> {
    let list = value
        .as_array()
        .ok_or("`notRun` must be an array of command ids: the commands that do not end on their own")?;
    let mut ids: Vec<String> = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let id = item
            .as_str()
            .filter(|id| is_command_id(&id.split(' ').map(str::to_string).collect::<Vec<_>>()))
            .ok_or_else(|| format!("`notRun/{index}` must be a command id (lower kebab-case words separated by one space)"))?;
        if ids.iter().any(|earlier| earlier == id) {
            return Err(format!("`notRun/{index}` repeats {id}"));
        }
        if probe.is_some_and(|probe| probe.join(" ") == id) {
            return Err(format!("`notRun/{index}` names {id}, which is the `probe`: the probe is a command that ends and runs without state"));
        }
        ids.push(id.to_string());
    }
    Ok(ids)
}

/// One argv array of strings at `path`, not empty.
fn argv(value: &Value, path: &str) -> Result<Vec<String>, String> {
    let words = value
        .as_array()
        .ok_or_else(|| format!("`{path}` must be an array of strings"))?;
    if words.is_empty() {
        return Err(format!("`{path}` must not be empty"));
    }
    words
        .iter()
        .enumerate()
        .map(|(at, word)| {
            word.as_str()
                .filter(|word| !word.contains('\0'))
                .map(str::to_string)
                .ok_or_else(|| format!("`{path}/{at}` must be a string"))
        })
        .collect()
}

/// An array of argv arrays at `path`.
fn argv_list(value: &Value, path: &str) -> Result<Vec<Vec<String>>, String> {
    let list = value
        .as_array()
        .ok_or_else(|| format!("`{path}` must be an array of argv arrays"))?;
    list.iter()
        .enumerate()
        .map(|(index, item)| argv(item, &format!("{path}/{index}")))
        .collect()
}

fn setup(value: &Value) -> Result<Vec<Vec<String>>, String> {
    argv_list(value, "setup")
}

fn env(value: &Value) -> Result<BTreeMap<String, String>, String> {
    let object = value
        .as_object()
        .ok_or("`env` must be an object of strings")?;
    object
        .iter()
        .map(|(name, value)| {
            if name.is_empty() || name.contains(['=', '\0']) {
                return Err(format!("`env` has the invalid variable name {name:?}"));
            }
            if name == "HOME" {
                return Err(
                    "`env` must not set HOME: the checker points it into the copy of the template"
                        .to_string(),
                );
            }
            let value = value
                .as_str()
                .filter(|text| !text.contains('\0'))
                .ok_or_else(|| format!("`env/{name}` must be a string"))?;
            Ok((name.clone(), value.to_string()))
        })
        .collect()
}

fn timeout(value: &Value) -> Result<Duration, String> {
    let seconds = value
        .as_f64()
        .filter(|seconds| *seconds > 0.0 && *seconds <= MAX_TIMEOUT_SECONDS);
    seconds.map(Duration::from_secs_f64).ok_or_else(|| {
        format!("`timeoutSeconds` must be a number above 0 and at most {MAX_TIMEOUT_SECONDS}")
    })
}

/// The command id pattern of the envelope schema's `meta.command`.
fn is_command_id(words: &[String]) -> bool {
    words.iter().all(|word| {
        let mut parts = word.split('-');
        let first_ok = parts.next().is_some_and(|part| {
            part.starts_with(|c: char| c.is_ascii_lowercase())
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
        first_ok
            && parts.all(|part| {
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            })
    })
}

fn probe(value: &Value) -> Result<Vec<String>, String> {
    let text = value
        .as_str()
        .ok_or("`probe` must be a string: the id of a command that runs without state")?;
    let words: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    if words.is_empty() || !is_command_id(&words) {
        return Err(format!(
            "`probe` {text:?} is not a command id (lower kebab-case words separated by spaces)"
        ));
    }
    Ok(words)
}

fn global_options(value: &Value) -> Result<Vec<Vec<String>>, String> {
    let options = argv_list(value, "globalOptions")?;
    for (index, option) in options.iter().enumerate() {
        if !option[0].starts_with("--") {
            return Err(format!(
                "`globalOptions/{index}` must start with a long option, as in [\"--repo\", \".\"]"
            ));
        }
    }
    Ok(options)
}

/// `UPPER_SNAKE_CASE`, the pattern of error codes, reasons and warning codes.
fn is_upper_snake(name: &str) -> bool {
    !name.is_empty()
        && name.split('_').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
        && name.starts_with(|c: char| c.is_ascii_uppercase())
}

/// A list of `UPPER_SNAKE_CASE` names at `path`, each once.
fn names(value: &Value, path: &str) -> Result<Vec<String>, String> {
    let list = value
        .as_array()
        .ok_or_else(|| format!("`{path}` must be an array of UPPER_SNAKE_CASE names"))?;
    let mut names: Vec<String> = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let name = item
            .as_str()
            .filter(|name| is_upper_snake(name))
            .ok_or_else(|| format!("`{path}/{index}` must be an UPPER_SNAKE_CASE name"))?;
        if names.iter().any(|earlier| earlier == name) {
            return Err(format!("`{path}/{index}` repeats {name}"));
        }
        names.push(name.to_string());
    }
    Ok(names)
}

fn warnings(value: &Value) -> Result<Vec<String>, String> {
    names(value, "warnings")
}

fn registry(value: &Value) -> Result<Vec<RegistryRow>, String> {
    let rows = value
        .as_array()
        .ok_or("`registry` must be an array of registry rows, or the string \"from-schema\"")?;
    let mut parsed: Vec<RegistryRow> = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let at = format!("registry/{index}");
        let object = row
            .as_object()
            .ok_or_else(|| format!("`{at}` must be an object {{code, exit, retryable, reasons?}}"))?;
        if let Some(member) = object
            .keys()
            .find(|key| !["code", "exit", "retryable", "reasons"].contains(&key.as_str()))
        {
            return Err(format!(
                "`{at}/{member}` is not a member of a fixture registry row: the fixture names codes and reasons and carries no prose"
            ));
        }
        let code = object
            .get("code")
            .and_then(Value::as_str)
            .filter(|code| is_upper_snake(code))
            .ok_or_else(|| format!("`{at}/code` must be an UPPER_SNAKE_CASE code"))?
            .to_string();
        if parsed.iter().any(|earlier| earlier.code == code) {
            return Err(format!("`{at}` repeats the code {code}"));
        }
        let exit = object
            .get("exit")
            .and_then(Value::as_i64)
            .filter(|exit| (1..=2).contains(exit))
            .ok_or_else(|| format!("`{at}/exit` must be 1 or 2"))?;
        let retryable = object
            .get("retryable")
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("`{at}/retryable` must be a boolean"))?;
        let reasons = object
            .get("reasons")
            .map(|reasons| names(reasons, &format!("{at}/reasons")))
            .transpose()?;
        parsed.push(RegistryRow {
            code,
            exit,
            retryable,
            reasons,
        });
    }
    Ok(parsed)
}

/// `batchFailsAt` and `batchFailsFirst`: a list of `{input: [document...], index}`; the first one fails, so its `index` is 0.
fn batch_cases(value: Option<&Value>, id: &str, key: &str) -> Result<Vec<BatchCase>, String> {
    let Some(value) = value else { return Ok(Vec::new()) };
    let list = value.as_array().ok_or_else(|| format!("`commands/{id}/{key}` must be an array of {{input, index}}"))?;
    let mut cases = Vec::with_capacity(list.len());
    for (at, item) in list.iter().enumerate() {
        let path = format!("commands/{id}/{key}/{at}");
        let object = item.as_object().ok_or_else(|| format!("`{path}` must be an object {{input, index}}"))?;
        if let Some(member) = object.keys().find(|member| !["input", "index"].contains(&member.as_str())) {
            return Err(format!("`{path}/{member}` is not a member of a batch entry: only `input` and `index`"));
        }
        let input = object
            .get("input")
            .and_then(Value::as_array)
            .filter(|documents| !documents.is_empty() && documents.iter().all(Value::is_object))
            .ok_or_else(|| format!("`{path}/input` must be a non-empty array of JSON objects: the documents of the batch"))?
            .clone();
        let index = object
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .filter(|index| *index < input.len())
            .ok_or_else(|| format!("`{path}/index` must be the index of an item of `input`"))?;
        if key == "batchFailsFirst" && index != 0 {
            return Err(format!("`{path}/index` must be 0 in `batchFailsFirst`: the first item fails"));
        }
        cases.push(BatchCase { input, index });
    }
    Ok(cases)
}

fn commands(value: &Value) -> Result<BTreeMap<String, CommandEntry>, String> {
    let object = value
        .as_object()
        .ok_or("`commands` must be an object keyed by command id")?;
    let mut entries = BTreeMap::new();
    for (id, entry) in object {
        let words: Vec<String> = id.split(' ').map(str::to_string).collect();
        if !is_command_id(&words) {
            return Err(format!(
                "`commands` has the key {id:?}, which is not a command id (lower kebab-case words separated by one space)"
            ));
        }
        let entry = entry
            .as_object()
            .ok_or_else(|| format!("`commands/{id}` must be an object"))?;
        let list = |key: &str| {
            entry
                .get(key)
                .map(|value| argv_list(value, &format!("commands/{id}/{key}")))
                .transpose()
                .map(Option::unwrap_or_default)
        };
        entries.insert(
            id.clone(),
            CommandEntry {
                ok: list("ok")?,
                refused: list("refused")?,
                missing_value: list("missingValue")?,
                fields: list("fields")?,
                paged: list("paged")?,
                domain_rejects: domain_rejects(entry.get("domainRejects"), id)?,
                batch_fails_at: batch_cases(entry.get("batchFailsAt"), id, "batchFailsAt")?,
                batch_fails_first: batch_cases(entry.get("batchFailsFirst"), id, "batchFailsFirst")?,
                preview: list("preview")?,
                async_entry: entry.get("async").map(|value| async_entry(value, id)).transpose()?,
                allowed_effects: allowed_effects(entry.get("allowedEffects"), id)?,
            },
        );
    }
    Ok(entries)
}

/// `async` of command `id`: `{start, worker, outcomes}`.
fn async_entry(value: &Value, id: &str) -> Result<AsyncEntry, String> {
    let path = format!("commands/{id}/async");
    let object = value.as_object().ok_or_else(|| format!("`{path}` must be an object {{start, worker, outcomes}}"))?;
    if let Some(member) = object.keys().find(|member| !["start", "worker", "outcomes"].contains(&member.as_str())) {
        return Err(format!("`{path}/{member}` is not a member of an async entry: only `start`, `worker` and `outcomes`"));
    }
    let start = argv(object.get("start").ok_or_else(|| format!("`{path}/start` is missing: the invocation that starts the work"))?, &format!("{path}/start"))?;
    let worker = object
        .get("worker")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty() && !name.contains(['=', '\0']))
        .ok_or_else(|| format!("`{path}/worker` must be the name of the environment variable the tool's test build reads to pick its worker's outcome"))?
        .to_string();
    let outcomes: Vec<String> = object
        .get("outcomes")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("`{path}/outcomes` must be an array of done, failed and timeout"))?
        .iter()
        .enumerate()
        .map(|(at, outcome)| {
            outcome
                .as_str()
                .filter(|outcome| ["done", "failed", "timeout"].contains(outcome))
                .map(str::to_string)
                .ok_or_else(|| format!("`{path}/outcomes/{at}` must be done, failed or timeout"))
        })
        .collect::<Result<_, _>>()?;
    if outcomes.is_empty() {
        return Err(format!("`{path}/outcomes` must name at least one outcome"));
    }
    Ok(AsyncEntry { start, worker, outcomes })
}

/// `allowedEffects` of command `id`: a list of non-empty glob patterns.
fn allowed_effects(value: Option<&Value>, id: &str) -> Result<Vec<String>, String> {
    let Some(value) = value else { return Ok(Vec::new()) };
    let list = value.as_array().ok_or_else(|| format!("`commands/{id}/allowedEffects` must be an array of path patterns"))?;
    list.iter()
        .enumerate()
        .map(|(at, pattern)| {
            pattern
                .as_str()
                .filter(|pattern| !pattern.is_empty() && !pattern.contains('\0'))
                .map(str::to_string)
                .ok_or_else(|| format!("`commands/{id}/allowedEffects/{at}` must be a non-empty path pattern"))
        })
        .collect()
}

/// `search`: a list of `{query, expect}`, `expect` a command id.
fn search(value: &Value) -> Result<Vec<SearchQuery>, String> {
    let list = value.as_array().ok_or("`search` must be an array of {query, expect}")?;
    list.iter()
        .enumerate()
        .map(|(at, item)| {
            let path = format!("search/{at}");
            let object = item.as_object().ok_or_else(|| format!("`{path}` must be an object {{query, expect}}"))?;
            if let Some(member) = object.keys().find(|member| !["query", "expect"].contains(&member.as_str())) {
                return Err(format!("`{path}/{member}` is not a member of a search entry: only `query` and `expect`"));
            }
            let query = object.get("query").and_then(Value::as_str).filter(|query| !query.trim().is_empty() && !query.contains('\0')).ok_or_else(|| format!("`{path}/query` must be a non-empty string"))?;
            let expect = object
                .get("expect")
                .and_then(Value::as_str)
                .filter(|id| is_command_id(&id.split(' ').map(str::to_string).collect::<Vec<_>>()))
                .ok_or_else(|| format!("`{path}/expect` must be a command id"))?;
            Ok(SearchQuery { query: query.to_string(), expect: expect.to_string() })
        })
        .collect()
}

/// `domainRejects` of command `id`: an object of argument name to a list of values (strings, as argv words).
fn domain_rejects(value: Option<&Value>, id: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| format!("`commands/{id}/domainRejects` must be an object of argument name to a list of values"))?;
    let mut rejects = BTreeMap::new();
    for (name, values) in object {
        let at = format!("commands/{id}/domainRejects/{name}");
        if name.is_empty() || name.starts_with('-') {
            return Err(format!("`{at}`: the key is an argument name without dashes (`limit`, not `--limit`)"));
        }
        let list = values
            .as_array()
            .filter(|list| !list.is_empty())
            .ok_or_else(|| format!("`{at}` must be a non-empty array of values"))?;
        let strings = list
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_str()
                    .filter(|text| !text.contains('\0'))
                    .map(str::to_string)
                    .ok_or_else(|| format!("`{at}/{index}` must be a string (a JSON value is written as its text)"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        rejects.insert(name.clone(), strings);
    }
    Ok(rejects)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse, DEFAULT_TIMEOUT};
    use crate::claim::Claim;

    #[test]
    fn atip_check_29_the_l0_keys_are_read_and_the_others_are_ignored() {
        let fixture = parse(&json!({
            "contractVersion": "1.1",
            "setup": [["init", "--json"]],
            "env": {"X": "1"},
            "timeoutSeconds": 5,
            "probe": "member list",
            "globalOptions": [["--repo", "."]],
            "registry": [
                {"code": "USAGE", "exit": 2, "retryable": false},
                {"code": "INPUT_INVALID", "exit": 2, "retryable": false, "reasons": ["NOT_JSON"]}
            ],
            "warnings": ["LEGACY_ALIAS"],
            "commands": {
                "finding close": {
                    "ok": [["finding", "close", "x", "--json"]],
                    "missingValue": [["finding", "close", "--json"]],
                    "fields": [["round"]], "domainRejects": {"tag": ["x"]}, "batchFailsAt": [{"input": [{}], "index": 0}], "preview": [["purge", "--json"]], "allowedEffects": ["a/*"], "async": {"start": ["deploy", "start"], "worker": "W", "outcomes": ["done"]}
                }
            },
            "search": [{"query": "close it", "expect": "finding close"}], "other": true
        }))
        .expect("valid");
        assert_eq!(fixture.claim(), Claim::V1_1);
        assert_eq!(fixture.setup, [["init", "--json"]]);
        assert_eq!(fixture.env["X"], "1");
        assert_eq!(fixture.timeout.as_secs(), 5);
        assert_eq!(fixture.probe_id().as_deref(), Some("member list"));
        assert_eq!(fixture.global_options, Some(vec![vec!["--repo".to_string(), ".".to_string()]]));
        assert_eq!(fixture.row("USAGE").map(|row| row.exit), Some(2));
        assert_eq!(fixture.row("INPUT_INVALID").and_then(|row| row.reasons.clone()), Some(vec!["NOT_JSON".to_string()]));
        assert_eq!(fixture.warnings, Some(vec!["LEGACY_ALIAS".to_string()]));
        assert_eq!(fixture.group().as_deref(), Some("finding"));
        assert_eq!(fixture.missing_values().len(), 1);
        let entry = &fixture.commands.as_ref().expect("commands")["finding close"];
        assert_eq!(entry.ok.len(), 1);
        assert_eq!((entry.preview.len(), entry.allowed_effects.len(), entry.async_entry.as_ref().map(|entry| entry.worker.as_str())), (1, 1, Some("W")));
        assert_eq!(fixture.search, [super::SearchQuery { query: "close it".to_string(), expect: "finding close".to_string() }]);
        let empty = parse(&json!({})).expect("an empty fixture is valid");
        assert_eq!(
            (empty.timeout, empty.probe.is_none(), empty.registry.is_none(), empty.claim()),
            (DEFAULT_TIMEOUT, true, true, Claim::V1_0)
        );
    }

    #[test]
    fn atip_check_27_a_fixture_of_the_wrong_shape_names_the_member() {
        let row = |extra: serde_json::Value| {
            let mut row = json!({"code": "USAGE", "exit": 2, "retryable": false});
            for (key, value) in extra.as_object().expect("object") {
                row[key] = value.clone();
            }
            json!({"registry": [row]})
        };
        for (document, needle) in [
            (json!([]), "object"),
            (json!({"contractVersion": "1.7"}), "contractVersion"),
            (json!({"contractVersion": 1.1}), "contractVersion"),
            (json!({"setup": "init"}), "setup"),
            (json!({"setup": [[]]}), "setup/0"),
            (json!({"setup": [["init", 1]]}), "setup/0/1"),
            (json!({"env": {"A": 1}}), "env/A"),
            (json!({"env": {"HOME": "/x"}}), "HOME"),
            (json!({"timeoutSeconds": 0}), "timeoutSeconds"),
            (json!({"timeoutSeconds": "5"}), "timeoutSeconds"),
            (json!({"probe": 3}), "probe"),
            (json!({"probe": "Status"}), "probe"),
            (json!({"probe": "  "}), "probe"),
            (json!({"globalOptions": "--repo"}), "globalOptions"),
            (json!({"globalOptions": [["repo", "."]]}), "globalOptions/0"),
            (json!({"registry": {}}), "registry"),
            (row(json!({"code": "usage"})), "registry/0/code"),
            (row(json!({"exit": 3})), "registry/0/exit"),
            (row(json!({"retryable": "no"})), "registry/0/retryable"),
            (row(json!({"meaning": "m"})), "registry/0/meaning"),
            (row(json!({"fix": "f"})), "registry/0/fix"),
            (row(json!({"reasons": "NOT_JSON"})), "registry/0/reasons"),
            (row(json!({"reasons": ["not_json"]})), "registry/0/reasons/0"),
            (row(json!({"reasons": ["A", "A"]})), "repeats"),
            (
                json!({"registry": [{"code": "USAGE", "exit": 2, "retryable": false}, {"code": "USAGE", "exit": 2, "retryable": false}]}),
                "repeats",
            ),
            (json!({"warnings": "X"}), "warnings"),
            (json!({"warnings": ["x"]}), "warnings/0"),
            (json!({"warnings": ["X", "X"]}), "repeats"),
            (json!({"commands": []}), "commands"),
            (json!({"commands": {"Finding": {}}}), "Finding"),
            (json!({"commands": {"finding": 1}}), "commands/finding"),
            (json!({"commands": {"finding": {"missingValue": ["a"]}}}), "commands/finding/missingValue/0"),
            (json!({"commands": {"finding": {"ok": [[]]}}}), "commands/finding/ok/0"),
        ] {
            let reason = parse(&document).expect_err(&format!("{document} must be refused"));
            assert!(reason.contains(needle), "{reason:?} must name {needle:?}");
        }
    }
}
