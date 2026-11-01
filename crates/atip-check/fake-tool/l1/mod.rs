//! The L1 part of the fake tool: a model of its commands (`spec`) from which the documents `schema` prints
//! (`doc`), `--help` (`help`) and the enforcement at run time (`parse`) are all derived, the generic seeded patch
//! of a printed document (`patch`), field selection (`project`) and the handlers (`run`).

mod doc;
mod help;
mod parse;
mod patch;
mod project;
mod run;
mod spec;

pub use doc::patched;
pub use run::handle;
pub use spec::MAX_INPUT_BYTES;

/// The registry rows an L1 fake adds to the L0 ones: code, meaning and fix (every one of them exits 2 and is not
/// retryable). `{tool}` stands for the tool name in the fix.
pub const EXTRA_ERRORS: [(&str, &str, &str); 5] = [
    (
        "DATE_INVALID",
        "A date argument names a day that is not on the calendar.",
        "Give an existing calendar date as YYYY-MM-DD.",
    ),
    (
        "INPUT_CONFLICT",
        "A value of the --input document and the same argument on the command line differ.",
        "Give the argument once, or give the same value both ways.",
    ),
    (
        "INPUT_TOO_LARGE",
        "The --input document is larger than the command accepts.",
        "Shorten the document.",
    ),
    (
        "MEMBER_NOT_FOUND",
        "The member does not exist.",
        "List the members with: {tool} member list --json",
    ),
    (
        "SORT_CONFLICT",
        "The sort order cannot be combined with another option.",
        "Drop the option or choose another sort order.",
    ),
];

use serde_json::{Map, Value};

use crate::Tool;

/// Applies the seeded patches of the knob `knob` (`dataPatch`, `metaPatch`: a map from a command id to operations).
fn apply_knob(tool: &Tool, knob: &str, command: &str, target: &mut Value) {
    let Some(ops) = tool
        .config
        .get(knob)
        .and_then(|patches| patches.get(command))
        .and_then(Value::as_array)
    else {
        return;
    };
    if let Err(reason) = patch::apply(target, ops) {
        eprintln!("fake tool: {knob} {command}: {reason}");
        std::process::exit(70);
    }
}

/// The seeded violations of a successful answer: `dataPatch` changes its `data`, `metaPatch` its `meta`.
pub fn seed_answer(tool: &Tool, command: &str, data: &mut Value, meta: &mut Map<String, Value>) {
    apply_knob(tool, "dataPatch", command, data);
    let mut as_value = Value::Object(std::mem::take(meta));
    apply_knob(tool, "metaPatch", command, &mut as_value);
    if let Value::Object(members) = as_value {
        *meta = members;
    }
}
