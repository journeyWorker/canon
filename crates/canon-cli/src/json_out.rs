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
/// An invocation of a deprecated form carries `meta.deprecated`
/// ([`crate::deprecation`]).
pub fn envelope(command: &str, data: Value) -> Value {
    let mut meta = json!({ "command": command });
    if let Some(deprecated) = crate::deprecation::meta() {
        meta["deprecated"] = deprecated;
    }
    json!({ "data": data, "meta": meta })
}

/// Prints the success envelope on stdout.
pub fn data(command: &str, data: Value) {
    println!("{}", envelope(command, data));
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
