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
/// A write command carries `meta.next` (0.21 D2); here it is empty, for a
/// command with no meaningful next step. [`data_with_next`] fills it.
pub fn envelope(command: &str, data: Value) -> Value {
    let mut meta = json!({ "command": command });
    if writes(command) {
        meta["next"] = json!([]);
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
    let mut envelope = envelope(command, data);
    if let (Some(meta), Some(extra)) = (envelope["meta"].as_object_mut(), after.meta().as_object()) {
        meta.extend(extra.clone());
    }
    println!("{envelope}");
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
