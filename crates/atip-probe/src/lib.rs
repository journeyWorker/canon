//! The smallest tool built on `atip`, as a tool that does not validate documents would be: it writes a next-step
//! command, splits it back, and refuses an argument outside its domain. It uses the crate with its default features,
//! so `cargo tree -p atip-probe` is the proof that such a tool does not link `jsonschema` (`tests/dependency_tree.rs`).

use serde_json::Value;

/// A next step's `command` string: the tool name and the words of `argv`, each quoted as the contract advises (spec 6.8).
pub fn next_command(tool: &str, argv: &[&str]) -> String {
    std::iter::once(tool).chain(argv.iter().copied()).map(atip::quote).collect::<Vec<_>>().join(" ")
}

/// The words of a `command` string, as a POSIX shell would read them.
pub fn words(command: &str) -> Vec<String> {
    atip::split_argv(command)
}

/// Whether `value` is inside `domain` (spec 10.7.4), judged by the crate itself.
pub fn refuses(domain: &Value, value: &Value) -> Result<bool, String> {
    atip::domain::admits(domain, value).map(|admitted| !admitted)
}

/// The reserved exit of `code` in a document that declares the newest minor the crate knows.
pub fn reserved_exit(code: &str) -> Option<u8> {
    atip::reserved_code(code, atip::Minor::LATEST).map(|reserved| reserved.exit)
}
