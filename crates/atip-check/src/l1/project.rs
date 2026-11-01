//! Field paths and projection (spec 6.10, 10.13 rules 11 and 15).
//!
//! OWNER: the rules agent. The signatures below are the contract the checks are written against.

use serde_json::Value;

use super::model::{CommandDoc, Output};

/// One step of a field path: a member, whether `[]` follows it, whether it was written as a bracketed key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub array: bool,
    pub bracket: bool,
}

/// The steps of a field path (spec 6.10), `None` when it is malformed.
pub fn parse_field_path(path: &str) -> Option<Vec<Step>> {
    let _ = path;
    None
}

/// Spec 10.13 rule 15: why `path` is malformed, or writes an identifier in brackets; `None` when it is well formed.
pub fn field_path_problem(path: &str) -> Option<String> {
    let _ = path;
    None
}

/// The records of `data` for `output`: the elements of the array at `fields.records` (an RFC 6901 pointer), or the one value there.
/// Empty when the output declares no `fields` or the pointer leads nowhere.
pub fn records<'a>(data: &'a Value, output: &Output) -> Vec<&'a Value> {
    let _ = (data, output);
    Vec::new()
}

/// Members the projected `data` holds that no kept path covers and that are not an ancestor of one (spec 6.10 rules 2-3), as `/pointer` paths from the record.
pub fn leaks(data: &Value, output: &Output, kept: &[String]) -> Vec<String> {
    let _ = (data, output, kept);
    Vec::new()
}

/// Kept members the unprojected `data` has and the projected one lacks, as `/pointer` paths (spec 6.10 rule 4), record by record;
/// empty when the two do not have the same number of records.
pub fn dropped(unprojected: &Value, projected: &Value, output: &Output, kept: &[String]) -> Vec<String> {
    let _ = (unprojected, projected, output, kept);
    Vec::new()
}

/// The output's schema with `required` reduced in every object cut node at or below the record along `kept` (spec 6.10, "Projected schema").
pub fn projected_validator(doc: &CommandDoc, output: &Output, kept: &[String]) -> Result<jsonschema::Validator, String> {
    let _ = (doc, output, kept);
    Err(String::new())
}

/// Whether `path` is selectable: declared in `fields.paths` or `fields.keep`, or, for a document that declares 1.1, one key of a declared map (spec 6.10).
pub fn selectable(doc: &CommandDoc, output: &Output, path: &str, v11: bool) -> bool {
    let _ = (doc, output, path, v11);
    false
}
