//! The plan of a destructive command, its digest, and the commit gate (spec 12.2-12.8, 8.3.2; approaches/rust.md 3).
//!
//! A destructive command run without `--commit` is a **preview**: it computes the [`Plan`], makes no effect, and answers with a
//! [`Preview`] (`planDigest`, `effects`, `committed: false`) whose first `meta.next` entry is the exact commit call
//! ([`commit_next`]). Run with `--commit --plan-digest <digest>` it recomputes the plan and goes through [`gate`]: the same
//! digest passes, another is `PLAN_STALE` (exit 2, nothing changed, `meta.next` the preview), and one of the two options without
//! the other is refused by [`mode`] (`USAGE`, the tool's finer code, or the code of the `requires` rule, [`COMMIT_REQUIRES`]).
//!
//! The digest is `sha256:` and the lower-case hex SHA-256 of the RFC 8785 canonical JSON ([`crate::jcs`]) of
//! `{"effects": [...], "inputs": {...}}` (spec 12.6). The effects are in the order the tool lists them; `inputs` is what the tool
//! says determines them: the normalised arguments and a digest of every state and file content the plan read. The crate does not
//! look inside `inputs`, so the same plan has the same digest in every tool that agrees on its `inputs`, and a plan that left
//! something out of it would not be refused when that changes. When another system computes the digest, the tool passes it through
//! and uses only [`gate`] (spec 12.6).
//!
//! The crate has no code that makes an effect: a preview and a refusal make none (spec 12.2) because nothing here can.

use std::collections::BTreeMap;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::constraint::Rule;
use crate::error::{CliError, CodeInfo};
use crate::given::{write_argv, Given};
use crate::jcs;
use crate::minor::Minor;
use crate::reserved::reserved_code;

/// The option that makes a destructive command act.
pub const COMMIT: &str = "commit";
/// The option that carries the digest of the plan to act on.
pub const PLAN_DIGEST: &str = "plan-digest";
/// The prefix of every digest.
pub const DIGEST_PREFIX: &str = "sha256:";

/// `--commit` and `--plan-digest` require each other (spec 12.7): the two `requires` rules a command document lists for them. A tool
/// adds its own `code` to a rule when it registers a finer code than `USAGE` for it (spec 10.8.1).
pub const COMMIT_REQUIRES: [Rule; 2] = [Rule::requires(COMMIT, &[PLAN_DIGEST]), Rule::requires(PLAN_DIGEST, &[COMMIT])];

/// One thing a commit would do (spec 12.4): the `action` and its target, precise enough that a caller can decide from the
/// effects alone. `path` is a file (relative to the tool's root), `record` a record that is not a file, `detail` where it goes or
/// why. A tool may add members of its own ([`Effect::with`]); they are part of the digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    /// What happens: `delete`, `move`, `upload`, `rewrite`, `append`, ...
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// Where it goes, or why: a sentence, or a JSON value the tool structures (`{"from": 1, "to": 2}`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
    /// The tool's own members, written after the contract's.
    #[serde(flatten, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(skip)]
    extensions: BTreeMap<String, Value>,
}

impl Effect {
    /// An effect that names no target.
    pub fn new(action: impl Into<String>) -> Self {
        Effect { action: action.into(), path: None, record: None, detail: None, extensions: BTreeMap::new() }
    }

    /// An effect on a file.
    pub fn path(action: impl Into<String>, path: impl Into<String>) -> Self {
        Effect { path: Some(path.into()), ..Effect::new(action) }
    }

    /// An effect on a record that is not a file.
    pub fn record(action: impl Into<String>, record: impl Into<String>) -> Self {
        Effect { record: Some(record.into()), ..Effect::new(action) }
    }

    /// Where it goes, or why, when the action alone does not say.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<Value>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A member of the tool's own. A name the contract has (`action`, `path`, `record`, `detail`) is not one a tool can replace.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        let name = name.into();
        if !["action", "path", "record", "detail"].contains(&name.as_str()) {
            self.extensions.insert(name, value.into());
        }
        self
    }
}

/// What a commit would do and what it rests on: the thing the digest covers (spec 12.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Every effect the commit would make, in the order it makes them (the order is part of the digest).
    pub effects: Vec<Effect>,
    /// What determines the effects: the normalised arguments, and a digest of each state and file content the plan read. A JSON
    /// object whose content the tool decides; the crate does not look inside it.
    pub inputs: Map<String, Value>,
}

impl Plan {
    pub fn new(effects: Vec<Effect>, inputs: Map<String, Value>) -> Self {
        Plan { effects, inputs }
    }

    /// The digest of the plan, or the number it holds that a digest cannot (see [`crate::jcs`]).
    pub fn digest(&self) -> Result<PlanDigest, jcs::Error> {
        digest_of(&serde_json::to_value(self).expect("a plan serializes"))
    }

    /// The report of a preview of this plan.
    pub fn preview(&self) -> Result<Preview, jcs::Error> {
        Ok(Preview { plan_digest: self.digest()?.0, effects: self.effects.clone(), committed: false })
    }
}

/// The digest of a plan written as a JSON document: `sha256:` and the lower-case hex SHA-256 of its RFC 8785 text (spec 12.6). For a
/// plan whose shape is the tool's own (the digest covers whatever document a tool builds), and what [`Plan::digest`] is made of. The
/// document holds only what JCS writes the same everywhere: an integer beyond ±(2^53-1) goes in as a string (an [`jcs::Error`]
/// otherwise), and a string holds valid Unicode, which a Rust `str` is.
pub fn digest_of(plan: &Value) -> Result<PlanDigest, jcs::Error> {
    Ok(PlanDigest(format!("{DIGEST_PREFIX}{}", jcs::sha256_hex(jcs::to_string(plan)?.as_bytes()))))
}

/// A digest as `sha256:` and 64 lower-case hex digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDigest(String);

impl PlanDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlanDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The `data` of a destructive command's answer (spec 12.4): `planDigest`, `effects`, `committed`. A tool's report schema can
/// reference [`preview_schema`] and add members of its own beside these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// The digest of the plan (`sha256:` and 64 lower-case hex digits); `--commit --plan-digest` repeats it.
    pub plan_digest: String,
    /// Every effect the commit would make, in the order it makes them.
    pub effects: Vec<Effect>,
    /// `false` for a preview, `true` after the commit.
    pub committed: bool,
}

impl Preview {
    /// The same report, marked as acted on.
    #[must_use]
    pub fn committed(mut self) -> Self {
        self.committed = true;
        self
    }
}

/// The JSON Schema of [`Preview`] (draft 2020-12), for a tool's report schema to reference.
pub fn preview_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(Preview)).expect("a schema serializes")
}

/// One entry of `meta.next` (spec 6.8): the invocation as argument words without the executable, the same as one display string
/// that splits back to the tool name and the words, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Next {
    pub command: String,
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Next {
    /// The step `tool argv...`, with `command` written the way the contract advises (spec 6.8).
    pub fn new(tool: &str, argv: Vec<String>, reason: impl Into<String>) -> Self {
        let command = std::iter::once(tool).chain(argv.iter().map(String::as_str)).map(crate::quote).collect::<Vec<_>>().join(" ");
        Next { command, argv, reason: Some(reason.into()) }
    }
}

/// The arguments of the invocation without the options of this module: what the plan was computed from.
fn without_commit(given: &[Given]) -> Vec<Given> {
    given.iter().filter(|argument| ![COMMIT, PLAN_DIGEST, "json", "input"].contains(&argument.name.as_str())).cloned().collect()
}

/// The step that commits `digest` (spec 12.5): the normalised arguments `given` of the preview as argv options, then `--commit`,
/// `--plan-digest=<digest>` and `--json`, the operands after one `--`. The arguments are written from `given`, not from how the
/// call was made: one that read `--input -` or a file has its arguments as options and no `--input`, a default and an environment
/// value are not written (see [`write_argv`]), and a value that starts with `-` is `--name=value`. A secret is not an argument
/// (spec 16.1), so there is none to disclose; the crate does not check that.
pub fn commit_next(tool: &str, path: &[&str], given: &[Given], digest: &PlanDigest, reason: impl Into<String>) -> Next {
    let mut arguments = without_commit(given);
    arguments.push(Given::switch(COMMIT, true));
    arguments.push(Given::option(PLAN_DIGEST, digest.as_str()));
    arguments.push(Given::switch("json", true));
    Next::new(tool, write_argv(path, &arguments), reason)
}

/// The preview call that goes with `given`: the same arguments without `--commit` and `--plan-digest`, and `--json`.
pub fn preview_next(tool: &str, path: &[&str], given: &[Given], reason: impl Into<String>) -> Next {
    let mut arguments = without_commit(given);
    arguments.push(Given::switch("json", true));
    Next::new(tool, write_argv(path, &arguments), reason)
}

/// What a call asks of a destructive command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Neither option: compute the plan, make no effect.
    Preview,
    /// Both options: act when the plan's digest is the one given.
    Commit(String),
}

/// One of `--commit` and `--plan-digest` given without the other (spec 12.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unpaired {
    /// The option that is missing, with its dashes: `--plan-digest` or `--commit`.
    pub missing: &'static str,
}

impl Unpaired {
    /// The refusal: `USAGE` by default, or the code of the row when the tool registered a finer one, or the `code` of the `requires`
    /// rule that says the relation (spec 12.7, 10.8.1). `field` is the missing option.
    pub fn error(self, finer: Option<&CodeInfo>) -> CliError {
        let given = if self.missing == "--commit" { "--plan-digest" } else { "--commit" };
        let message = format!("{given} needs {}: a commit repeats the digest of the preview, and a digest is for a commit", self.missing);
        let error = match finer {
            Some(row) => CliError::from_row(row.code, message, Some(row)),
            None => reserved_error("USAGE", message),
        };
        error.field(self.missing).hint("run the command without them for a preview, then commit with the --commit and --plan-digest it prints")
    }
}

impl fmt::Display for Unpaired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is missing", self.missing)
    }
}

impl std::error::Error for Unpaired {}

/// The mode the options select: a preview for neither, a commit for both, [`Unpaired`] for one.
pub fn mode(commit: bool, plan_digest: Option<&str>) -> Result<Mode, Unpaired> {
    match (commit, plan_digest) {
        (false, None) => Ok(Mode::Preview),
        (true, Some(digest)) => Ok(Mode::Commit(digest.to_string())),
        (true, None) => Err(Unpaired { missing: "--plan-digest" }),
        (false, Some(_)) => Err(Unpaired { missing: "--commit" }),
    }
}

/// A commit whose digest is not the plan's (spec 12.7): `PLAN_STALE` (exit 2, nothing was changed) and the `meta.next` that goes
/// with it, the preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stale {
    pub error: CliError,
    /// The `meta.next` of the refusal: the preview call.
    pub next: Vec<Next>,
}

/// Whether a call acts: `Ok(false)` for a preview, `Ok(true)` for a commit of exactly the plan whose digest is `current`, and
/// `PLAN_STALE` for a commit of another. `current` is the digest of the plan the tool has just recomputed ([`Plan::digest`]), or
/// the one the system that makes the effects computed. The comparison is exact: a digest that differs in one character, or one
/// written without the `sha256:` prefix, is another digest. `preview` is the `meta.next` entry that leads back to the preview.
pub fn gate(mode: &Mode, current: &str, preview: Next) -> Result<bool, Stale> {
    match mode {
        Mode::Preview => Ok(false),
        Mode::Commit(given) if given == current => Ok(true),
        Mode::Commit(given) => {
            let error = reserved_error("PLAN_STALE", format!("the plan digest {given} no longer matches the plan (now {current}); nothing was changed"))
                .field("--plan-digest")
                .hint(format!("run `{}` again, review the new plan, then commit it with the digest it prints", preview.command));
            Err(Stale { error, next: vec![preview] })
        }
    }
}

/// A commit that stopped after some of its effects because the plan changed (spec 12.7, 8.3.2): `STOPPED_MIDWAY` (exit 1, not
/// retryable) with the reason `PLAN_STALE`, and a `meta.next` that leads with the command that reads what was done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    pub error: CliError,
    pub next: Vec<Next>,
}

/// The stop of a commit that met a changed plan after `done` effects of `of` were made. `what_was_done` is the command that reads
/// them (a status, a log); repeating the commit blindly could repeat the steps that ran, so it is not what the `next` leads with.
/// The registry of a tool that declares 1.2 lists `PLAN_STALE` among the reasons of `STOPPED_MIDWAY` ([`STOPPED_MIDWAY_REASON`]).
pub fn stopped_by_stale(done: usize, of: usize, what_was_done: Next) -> Stopped {
    let message = format!("the plan changed after {done} of {of} effects were made; the effects already made were not undone");
    let error = reserved_error("STOPPED_MIDWAY", message).reason(STOPPED_MIDWAY_REASON).hint(format!("read what was done with `{}` before running anything again", what_was_done.command));
    Stopped { error, next: vec![what_was_done] }
}

/// The `reason` of the `STOPPED_MIDWAY` error a changed plan gives: the name of the exit-2 code, `PLAN_STALE` (spec 8.3.2).
pub const STOPPED_MIDWAY_REASON: &str = "PLAN_STALE";

/// An error of the reserved code `name`, with the exit and retryability the contract gives it (spec 7.2.1): a code's row, not a
/// call site, chooses them (spec 7.3.2).
fn reserved_error(name: &'static str, message: String) -> CliError {
    let reserved = reserved_code(name, Minor::LATEST).expect("a reserved code");
    let row = CodeInfo { code: reserved.name, meaning: "", exit: reserved.exit, retryable: reserved.retryable, fix: "", reasons: &[] };
    CliError::from_row(reserved.name, message, Some(&row))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::constraint::{violation, Given as Seen};

    fn inputs(value: &Value) -> Map<String, Value> {
        value.as_object().expect("an object").clone()
    }

    fn plan() -> Plan {
        Plan::new(
            vec![Effect::path("delete", "a/old.txt").detail("older than 30 days"), Effect::record("remove", "member:7")],
            inputs(&json!({ "args": { "older-than": 30, "kind": "log" }, "read": { "members": "sha256:aa" } })),
        )
    }

    /// The same plan as a tool in another language writes it down, by hand: the effects in their order, every object's members sorted by name.
    const PLAN_TEXT: &str = r#"{"effects":[{"action":"delete","detail":"older than 30 days","path":"a/old.txt"},{"action":"remove","record":"member:7"}],"inputs":{"args":{"kind":"log","older-than":30},"read":{"members":"sha256:aa"}}}"#;

    fn digest_of(plan: &Plan) -> String {
        plan.digest().expect("a digest").as_str().to_string()
    }

    fn preview_call() -> Vec<Given> {
        vec![Given::option("older-than", 30), Given::option("kind", "log"), Given::operands(vec![json!("logs")])]
    }

    #[test]
    fn atip_crateplan_01_the_digest_is_sha256_of_the_jcs_text_of_the_effects_and_inputs() {
        let digest = digest_of(&plan());
        let hex = digest.strip_prefix("sha256:").expect("the prefix");
        assert_eq!(hex.len(), 64);
        assert!(hex.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)), "{digest}");
        assert_eq!(hex, jcs::sha256_hex(PLAN_TEXT.as_bytes()), "the hex digits are the SHA-256 of {PLAN_TEXT}");
        assert_eq!(jcs::to_string(&serde_json::to_value(plan()).expect("a plan")).expect("canonical"), PLAN_TEXT);
        assert_eq!(plan().digest().expect("a digest").to_string(), digest);
        // Nothing is added: no effects and no inputs are the digest of `{"effects":[],"inputs":{}}`.
        assert_eq!(digest_of(&Plan::new(vec![], Map::new())), format!("sha256:{}", jcs::sha256_hex(br#"{"effects":[],"inputs":{}}"#)));
    }

    #[test]
    fn atip_crateplan_02_the_order_of_inputs_does_not_matter_and_every_other_change_does() {
        let base = digest_of(&plan());
        let reordered = Plan::new(plan().effects, inputs(&json!({ "read": { "members": "sha256:aa" }, "args": { "kind": "log", "older-than": 30 } })));
        assert_eq!(digest_of(&reordered), base, "keys in another order are the same inputs");
        // The order of the effects, and any value of one, change it.
        let mut swapped = plan();
        swapped.effects.reverse();
        assert_ne!(digest_of(&swapped), base);
        let mut other_target = plan();
        other_target.effects[0] = Effect::path("delete", "a/old.tx");
        assert_ne!(digest_of(&other_target), base, "one character of an effect");
        let mut other_action = plan();
        other_action.effects[1] = Effect::record("purge", "member:7");
        assert_ne!(digest_of(&other_action), base);
        let mut dropped = plan();
        dropped.effects.pop();
        assert_ne!(digest_of(&dropped), base);
        // Every input counts: one character of the state digest, one argument, a key added, a null kept.
        for changed in [
            json!({ "args": { "older-than": 30, "kind": "log" }, "read": { "members": "sha256:ab" } }),
            json!({ "args": { "older-than": 31, "kind": "log" }, "read": { "members": "sha256:aa" } }),
            json!({ "args": { "older-than": 30, "kind": "log", "dry": false }, "read": { "members": "sha256:aa" } }),
            json!({ "args": { "older-than": 30, "kind": "log" }, "read": { "members": "sha256:aa" }, "files": null }),
            json!({ "args": { "older-than": 30, "kind": "log" }, "read": { "members": "sha256:aa" }, "files": [] }),
        ] {
            assert_ne!(digest_of(&Plan::new(plan().effects, inputs(&changed))), base, "{changed}");
        }
        // The tool's own members of an effect are covered too, and `1` and `1.0` are one value.
        let mut extended = plan();
        extended.effects[0] = extended.effects[0].clone().with("bytes", 12);
        assert_ne!(digest_of(&extended), base);
        let as_float = Plan::new(plan().effects, inputs(&json!({ "args": { "older-than": 30.0, "kind": "log" }, "read": { "members": "sha256:aa" } })));
        assert_eq!(digest_of(&as_float), base);
        // A member the contract names is not one a tool can replace.
        assert_eq!(Effect::path("delete", "a").with("path", "b").with("action", "x").with("record", "r").with("detail", "d"), Effect::path("delete", "a"));
    }

    #[test]
    fn atip_crateplan_03_an_integer_a_digest_cannot_hold_is_an_error_and_a_fraction_is_a_double() {
        for number in [json!(9_007_199_254_740_993_u64), json!(-9_007_199_254_740_993_i64), json!(18_446_744_073_709_551_615_u64)] {
            let in_inputs = Plan::new(vec![], inputs(&json!({ "n": number })));
            assert!(matches!(in_inputs.digest(), Err(jcs::Error::NumberNotSafe(_))), "{number}");
            assert!(in_inputs.preview().is_err(), "{number}");
            let in_effect = Plan::new(vec![Effect::new("delete").with("size", number.clone())], Map::new());
            assert!(in_effect.digest().is_err(), "{number}");
        }
        Plan::new(vec![], inputs(&json!({ "n": 9_007_199_254_740_991_i64 }))).digest().expect("the largest safe integer is held");
        // As a string it is held, digits unchanged; a fraction is the double RFC 8785 writes.
        let as_string = Plan::new(vec![], inputs(&json!({ "n": "9007199254740993" }))).digest().expect("a string is held");
        assert_eq!(as_string.as_str(), format!("sha256:{}", jcs::sha256_hex(br#"{"effects":[],"inputs":{"n":"9007199254740993"}}"#)));
        let fraction = Plan::new(vec![], inputs(&json!({ "n": 1.5 }))).digest().expect("a fraction is held");
        assert_eq!(fraction.as_str(), format!("sha256:{}", jcs::sha256_hex(br#"{"effects":[],"inputs":{"n":1.5}}"#)));
    }

    #[test]
    fn atip_crateplan_04_the_preview_report_has_the_three_members_and_passes_the_crates_schema() {
        let preview = plan().preview().expect("a preview");
        assert_eq!(preview.plan_digest, digest_of(&plan()));
        assert!(!preview.committed);
        let data = serde_json::to_value(&preview).expect("a report");
        assert_eq!(data, json!({ "planDigest": digest_of(&plan()), "effects": [{ "action": "delete", "path": "a/old.txt", "detail": "older than 30 days" }, { "action": "remove", "record": "member:7" }], "committed": false }));
        let schema = preview_schema();
        let validator = jsonschema::validator_for(&schema).expect("the schema compiles");
        assert!(validator.is_valid(&data), "{data}");
        assert!(validator.is_valid(&serde_json::to_value(preview.clone().committed()).expect("a report")), "committed: true is the report of the commit");
        assert!(preview.committed().committed);
        // A tool's own members of an effect pass too; a report that lacks a member, or has another type, does not.
        let extended = serde_json::to_value(plan().preview().map(|mut p| {
            p.effects[0] = p.effects[0].clone().with("bytes", 12);
            p
        })
        .expect("a preview"))
        .expect("a report");
        assert!(validator.is_valid(&extended), "{extended}");
        for broken in [json!({ "effects": [], "committed": false }), json!({ "planDigest": "sha256:x", "committed": false }), json!({ "planDigest": "sha256:x", "effects": [], "committed": "no" }), json!({ "planDigest": "sha256:x", "effects": [{ "path": "a" }], "committed": false }), json!({ "planDigest": 1, "effects": [], "committed": false })] {
            assert!(!validator.is_valid(&broken), "{broken}");
        }
    }

    #[test]
    fn atip_crateplan_05_the_commit_step_is_the_normalised_arguments_commit_digest_and_json_and_splits_back() {
        let digest = plan().digest().expect("a digest");
        let step = commit_next("acme", &["log", "purge"], &preview_call(), &digest, "commit the plan this preview printed");
        assert_eq!(step.argv, ["log", "purge", "--older-than=30", "--kind=log", "--commit", &format!("--plan-digest={digest}"), "--json", "--", "logs"]);
        assert_eq!(step.command, format!("acme log purge --older-than=30 --kind=log --commit --plan-digest={digest} --json -- logs"));
        let mut words = vec!["acme".to_string()];
        words.extend(step.argv.clone());
        assert_eq!(crate::split_argv(&step.command), words, "the command splits back to the tool name and argv");
        assert_eq!(step.reason.as_deref(), Some("commit the plan this preview printed"));
        // As a document it is a valid `meta.next` entry and the first of a valid envelope.
        let envelope = json!({
            "ok": true,
            "data": plan().preview().expect("a preview"),
            "meta": { "schemaVersion": "1.0", "tool": "acme", "toolVersion": "1.0.0", "contract": "atip/1", "contractVersion": "1.2", "command": "log purge", "next": [step] }
        });
        assert_eq!(crate::envelope_problems(&envelope), Vec::<String>::new());
        // A word that needs quoting is quoted in `command` and literal in `argv`.
        let spaced = commit_next("acme", &["log", "purge"], &[Given::option("kind", "a b'c")], &digest, "r");
        assert_eq!(spaced.argv[2], "--kind=a b'c");
        assert!(spaced.command.contains("'--kind=a b'\\''c'"), "{}", spaced.command);
        let mut words = vec!["acme".to_string()];
        words.extend(spaced.argv.clone());
        assert_eq!(crate::split_argv(&spaced.command), words);
        // The preview step is the same call without the commit options.
        let preview = preview_next("acme", &["log", "purge"], &preview_call(), "run the preview again");
        assert_eq!(preview.argv, ["log", "purge", "--older-than=30", "--kind=log", "--json", "--", "logs"]);
    }

    #[test]
    fn atip_crateplan_06_arguments_that_came_by_input_or_with_a_dash_are_written_as_options_and_nothing_else_is() {
        let digest = plan().digest().expect("a digest");
        let given = vec![
            Given::option("input", "-"),
            Given::option("name", "-x"),
            Given::option("empty", ""),
            Given::option("role", "member").from_env(),
            Given::switch("quiet", false),
            Given::option("commit", "stale"),
            Given::option("plan-digest", "sha256:old"),
            Given::switch("json", true),
            Given::option("filter", json!({ "a": [1, 2] })),
            Given::operands(vec![json!("-dash"), json!("two")]),
        ];
        let step = commit_next("acme", &["x"], &given, &digest, "r");
        assert_eq!(step.argv, ["x", "--name=-x", "--empty=", "--filter={\"a\":[1,2]}", "--commit", &format!("--plan-digest={digest}"), "--json", "--", "-dash", "two"]);
        assert!(!step.argv.iter().any(|word| word == "--input" || word == "-" || word.starts_with("--role") || word.contains("stale") || word.contains("sha256:old") || word.starts_with("--quiet")), "{:?}", step.argv);
        assert_eq!(step.argv.iter().filter(|word| word.as_str() == "--json").count(), 1);
        assert_eq!(step.argv.iter().filter(|word| word.starts_with("--plan-digest")).count(), 1);
    }

    #[test]
    fn atip_crateplan_07_the_same_digest_commits_and_a_call_without_options_is_a_preview() {
        let current = plan().digest().expect("a digest");
        let back = preview_next("acme", &["x"], &[], "preview");
        assert_eq!(gate(&Mode::Commit(current.to_string()), current.as_str(), back.clone()), Ok(true));
        assert_eq!(gate(&Mode::Preview, current.as_str(), back.clone()), Ok(false));
        assert_eq!(mode(false, None), Ok(Mode::Preview));
        assert_eq!(mode(true, Some(current.as_str())), Ok(Mode::Commit(current.to_string())));
        // A preview never asks the gate to compare: a digest the preview did not get cannot make it stale.
        assert_eq!(gate(&Mode::Preview, "sha256:anything", back), Ok(false));
    }

    #[test]
    fn atip_crateplan_08_another_digest_is_plan_stale_exit_2_with_the_preview_as_next() {
        let current = plan().digest().expect("a digest");
        let back = preview_next("acme", &["log", "purge"], &preview_call(), "run the preview again, review the new plan and commit it");
        let mut one_character = current.to_string();
        one_character.pop();
        one_character.push(if current.as_str().ends_with('0') { '1' } else { '0' });
        let without_prefix = current.as_str().strip_prefix("sha256:").expect("the prefix").to_string();
        let mut changed = plan();
        changed.effects.pop();
        let other_plan = changed.digest().expect("a digest").to_string();
        for given in [one_character, without_prefix, other_plan, String::new(), current.as_str().to_uppercase(), format!(" {current}")] {
            let stale = gate(&Mode::Commit(given.clone()), current.as_str(), back.clone()).expect_err("a commit of another plan is refused");
            assert_eq!((stale.error.code, stale.error.exit, stale.error.retryable), ("PLAN_STALE", 2, false), "{given:?}");
            assert_eq!(stale.error.field_name(), Some("--plan-digest"));
            assert!(stale.error.hint_text().is_some_and(|hint| hint.contains("acme log purge --older-than=30")), "{:?}", stale.error.hint_text());
            assert_eq!(stale.next, vec![back.clone()], "the next is the preview call");
            assert!(!stale.next[0].argv.iter().any(|word| word.starts_with("--commit") || word.starts_with("--plan-digest")), "the preview is not a commit");
        }
        // The refusal is a value: the gate reads nothing and writes nothing, so a refusal changes nothing (spec 12.2, 12.7).
    }

    #[test]
    fn atip_crateplan_09_a_digest_another_system_computed_is_compared_as_it_is() {
        let remote = "sha256:3b4c9d0e-the-remote-format-is-the-remotes";
        let back = preview_next("acme", &["x"], &[], "preview");
        assert_eq!(gate(&Mode::Commit(remote.to_string()), remote, back.clone()), Ok(true));
        gate(&Mode::Commit(remote.to_string()), "sha256:other", back).expect_err("another digest is stale");
    }

    #[test]
    fn atip_crateplan_10_one_option_without_the_other_is_usage_or_the_tools_finer_code_with_the_missing_field() {
        let commit_only = mode(true, None).expect_err("--commit alone is refused");
        let digest_only = mode(false, Some("sha256:aa")).expect_err("--plan-digest alone is refused");
        assert_eq!((commit_only.missing, digest_only.missing), ("--plan-digest", "--commit"));
        let error = commit_only.error(None);
        assert_eq!((error.code, error.exit, error.retryable, error.field_name()), ("USAGE", 2, false, Some("--plan-digest")));
        assert!(error.to_string().contains("--commit needs --plan-digest"), "{error}");
        let error = digest_only.error(None);
        assert_eq!((error.code, error.field_name()), ("USAGE", Some("--commit")));
        // The tool's finer code is the code, with the exit and retryability of its row.
        const FINER: CodeInfo = CodeInfo { code: "PURGE_INVALID_ARGUMENT", meaning: "m", exit: 2, retryable: false, fix: "f", reasons: &[] };
        let error = commit_only.error(Some(&FINER));
        assert_eq!((error.code, error.exit, error.field_name()), ("PURGE_INVALID_ARGUMENT", 2, Some("--plan-digest")));
        assert!(Unpaired { missing: "--commit" }.to_string().contains("--commit"));
        // Both, or neither, is not refused; the two rules a command document lists say the same.
        assert!(mode(true, Some("sha256:aa")).is_ok() && mode(false, None).is_ok());
        let [commit_rule, digest_rule] = COMMIT_REQUIRES.map(|rule| rule.doc());
        let seen = |names: &[&str]| -> Seen { names.iter().map(|name| ((*name).to_string(), vec![json!(true)])).collect() };
        assert_eq!(violation(&commit_rule, &seen(&["commit"])), Some("plan-digest".to_string()));
        assert_eq!(violation(&digest_rule, &seen(&["plan-digest"])), Some("commit".to_string()));
        assert_eq!((violation(&commit_rule, &seen(&["commit", "plan-digest"])), violation(&digest_rule, &seen(&["commit", "plan-digest"])), violation(&commit_rule, &seen(&[]))), (None, None, None));
        assert_eq!((COMMIT, PLAN_DIGEST), ("commit", "plan-digest"));
    }

    #[test]
    fn atip_crateplan_11_a_changed_plan_after_some_effects_is_stopped_midway_with_the_reason_plan_stale() {
        let read = Next::new("acme", vec!["log".into(), "status".into(), "--json".into()], "read what the commit did before it stopped");
        let stopped = stopped_by_stale(2, 5, read.clone());
        let error = &stopped.error;
        assert_eq!((error.code, error.exit, error.retryable), ("STOPPED_MIDWAY", 1, false));
        assert_eq!(error.reason_name(), Some("PLAN_STALE"));
        assert_eq!(STOPPED_MIDWAY_REASON, "PLAN_STALE");
        assert!(error.to_string().contains("after 2 of 5"), "{error}");
        assert!(error.hint_text().is_some_and(|hint| hint.contains("acme log status --json")), "{:?}", error.hint_text());
        assert_eq!(stopped.next, vec![read], "the next leads with the command that reads what was done, not with a retry");
        // The exit is the reserved row's, and is never 2 (spec 8.2).
        assert_ne!(error.exit, 2);
        assert_eq!(crate::registration_problem(Minor::V1_2, "STOPPED_MIDWAY", Some(i64::from(error.exit)), Some(error.retryable)), None);
        assert_ne!(stopped.error.code, "PLAN_STALE", "a stop part-way is never the exit-2 code");
    }
}
