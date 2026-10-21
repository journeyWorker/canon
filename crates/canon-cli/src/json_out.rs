//! The `--json` output contract for commands that report a result (0.20 D2).
//!
//! In JSON mode stdout carries exactly one JSON document and nothing else:
//! a success is [`data`]'s `{ "data": …, "meta": { "command": "<path>" } }`,
//! a failure is [`crate::cli_error::CliError`]'s `{ "error": … }`. Every
//! human line a command would print goes to stderr instead ([`say!`]), so a
//! caller that reads stdout as JSON never trips over prose, and a log reader
//! still sees what happened.

use serde_json::{json, Value};

/// The success envelope for `command` (its canonical path, e.g. `gate check`).
/// A write command carries `meta.next` (0.21 D2);
/// here it is empty, for a command with no meaningful next step.
/// [`data_with_next`] fills it. An invocation that used a deprecated spelling
/// carries `meta.deprecated` ([`crate::deprecation`]).
pub fn envelope(command: &str, data: Value) -> Value {
    let mut meta = json!({ "command": command });
    if writes(command) {
        meta["next"] = json!([]);
    }
    if let Some(deprecated) = crate::deprecation::meta() {
        meta["deprecated"] = deprecated;
    }
    json!({ "data": data, "meta": meta })
}

/// Whether `command` (a canonical path) writes, per [`crate::schema::META`].
fn writes(command: &str) -> bool {
    crate::schema::META.iter().any(|row| row.path == command && row.effect != crate::schema::Effect::Read)
}

/// Prints the success envelope on stdout.
pub fn data(command: &str, data: Value) {
    println!("{}", envelope(command, data));
}

/// Prints a write's success envelope with the steps that follow it
/// ([`crate::next`]).
pub fn data_with_next(command: &str, data: Value, after: &crate::next::After) {
    data_with_next_and_meta(command, data, after, Value::Null);
}

/// [`data_with_next`] with extra `meta` keys (a page's `nextCursor`, …).
pub fn data_with_next_and_meta(command: &str, data: Value, after: &crate::next::After, extra_meta: Value) {
    println!("{}", envelope_with_next(command, data, after, extra_meta));
}

/// The write envelope [`data_with_next_and_meta`] prints, as a value: for a
/// caller that answers over a socket instead of stdout.
pub fn envelope_with_next(command: &str, data: Value, after: &crate::next::After, extra_meta: Value) -> Value {
    let mut envelope = envelope(command, data);
    if let Some(meta) = envelope["meta"].as_object_mut() {
        for extra in [&extra_meta, &after.meta()] {
            if let Some(extra) = extra.as_object() {
                meta.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
    }
    envelope
}

/// Splits a payload that carries its own page `meta` ([`crate::paging`]'s
/// `total`, `returned`, `nextCursor`) into `(data, meta)`, so the page
/// state lands in `meta` beside `command` instead of inside `data`.
pub fn split_meta(mut payload: Value) -> (Value, Value) {
    let meta = payload.as_object_mut().and_then(|object| object.remove("meta")).unwrap_or(Value::Null);
    (payload, meta)
}

/// [`data`] with extra `meta` keys beside `command` (`unchanged`, …).
pub fn data_with_meta(command: &str, data: Value, extra_meta: Value) {
    let mut body = envelope(command, data);
    if let (Some(meta), Some(extra)) = (body["meta"].as_object_mut(), extra_meta.as_object()) {
        meta.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    println!("{body}");
}

/// A command's narration: each line is said (stdout, or stderr under
/// `--json`) and kept, so the JSON result can carry the same account.
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

/// `println!` in human mode, `eprintln!` in JSON mode: `say!(json, "…", args)`.
#[macro_export]
macro_rules! say {
    ($json:expr, $($arg:tt)*) => {
        if $json { eprintln!($($arg)*); } else { println!($($arg)*); }
    };
}

/// `print!` in human mode, `eprint!` in JSON mode: `say_raw!(json, "…", args)`.
#[macro_export]
macro_rules! say_raw {
    ($json:expr, $($arg:tt)*) => {
        if $json { eprint!($($arg)*); } else { print!($($arg)*); }
    };
}
