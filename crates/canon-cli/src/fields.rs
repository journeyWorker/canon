//! `--fields` of the six outputs that declare field selection (atip/1 spec 6.10, 10.9).
//!
//! `finding list`, `divergence list`, `query`, `export`, `status` and `context` take `--fields <FIELDS>`: a
//! comma list of field paths their command document declares (`outputs[].fields`). Canon declares each output in
//! one row of [`OUTPUTS`] (where the records are, which paths are always kept) and reads the selectable paths off the
//! record schema of the report itself ([`resolve`]), so the declaration cannot name a member the report lacks.
//! `tests/atip_paging_fields.rs` holds every declaration to spec 10.13 rule 11 (projectable cut nodes, every path a
//! member of the record schema) and every projected answer to the schema with `required` reduced (spec 6.10).
//!
//! A selection is made once per run ([`select`], or [`choose`] for a command that projects more than the envelope's
//! `data`): it refuses a path the declaration does not hold (`USAGE`, `field` `--fields`, `expected` the selectable
//! paths) and otherwise becomes the run's projection. The envelope ([`crate::json_out`]) projects the records of
//! `data` after the report is serialized, and puts the kept paths in `meta.fields`. Paths are the record's own keys,
//! spelled once: there is no camelCase or snake_case alias.

use std::collections::BTreeMap;
use std::sync::Mutex;

use atip::doc::FieldsDoc;
use atip::fields::{Declared, FieldPath, Projection, Selection};
use serde_json::{json, Value};

use crate::cli_error::{CliError, CliErrorExt as _};

/// One output that declares `fields`.
#[derive(Debug)]
pub struct Output {
    pub command: &'static str,
    /// The name of the report schema of the output.
    pub report: &'static str,
    /// RFC 6901 pointer of the record array in `data`; `""` when `data` is the record.
    pub records: &'static str,
    /// The members of a record that every answer keeps.
    pub keep: &'static [&'static str],
}

/// The state members a report of a command that carries state may hold at its root ([`crate::json_out::state_properties`]).
const STATE: &[&str] = &["unchanged", "waitingOn", "nextOmitted"];

/// The outputs with `fields`: one per command, three for `export`.
pub const OUTPUTS: &[Output] = &[
    Output { command: "context", report: "ContextReport", records: "", keep: &[] },
    Output { command: "divergence list", report: "DivergenceListReport", records: "/scenarios", keep: &["projectId", "scenarioId"] },
    // `export` answers one of three shapes, each its own output with its own `fields` (spec 6.10): the manifest has records to cut,
    // the other two are their own record, like `status`.
    Output { command: "export", report: "ExportReport", records: "/records", keep: &[] },
    Output { command: "export", report: "ExportWrittenReport", records: "", keep: STATE },
    Output { command: "export", report: "ExportPreviewReport", records: "", keep: STATE },
    Output { command: "finding list", report: "FindingListReport", records: "/findings", keep: &["round", "seq"] },
    Output { command: "query", report: "QueryReport", records: "/records", keep: &[] },
    // `status` is its own record, so the members every write adds to its report stay whatever is selected.
    Output { command: "status", report: "StatusReport", records: "", keep: STATE },
];

/// An output's declaration with the schema it was read from.
#[derive(Debug)]
pub struct Resolved {
    /// What the command document publishes.
    pub fields: FieldsDoc,
    pub declared: Declared,
    /// The document the schema's `$ref`s are read in (`{"schemas": …}`) and the node of the whole output.
    pub doc: Value,
    pub root: Value,
}

/// The declaration of `output`: the members of its record schema, but the kept ones, are the selectable paths.
///
/// # Errors
/// When the report has no schema, or a kept member is not one the record declares: a defect of [`OUTPUTS`].
pub fn resolve(output: &Output) -> Result<Resolved, String> {
    let report = crate::reports::schemas_of(output.command).into_iter().find(|report| report.name == output.report).ok_or_else(|| format!("`{}` has no report `{}`", output.command, output.report))?;
    let mut schemas = BTreeMap::new();
    atip::project::add_schema(&mut schemas, output.report, report.schema)?;
    let doc = json!({ "schemas": schemas });
    let root = json!({ "$ref": format!("#/schemas/{}", output.report) });
    let members = atip::fields::member_names(&doc, &root, output.records);
    if let Some(missing) = output.keep.iter().find(|name| !members.iter().any(|member| member == *name)) {
        return Err(format!("`{}` keeps `{missing}`, which its record does not declare", output.command));
    }
    let spell = |name: &str| FieldPath::member(name).to_string();
    let paths: Vec<String> = members.iter().filter(|member| !output.keep.contains(&member.as_str())).map(|member| spell(member)).collect();
    let keep: Vec<String> = output.keep.iter().map(|name| spell(name)).collect();
    let parse = |list: &[String]| list.iter().map(|text| FieldPath::parse(text).map_err(|error| error.to_string())).collect::<Result<Vec<_>, _>>();
    let maps = atip::fields::declared_maps(&doc, &root, output.records, &paths);
    let declared = Declared { records: output.records.to_string(), paths: parse(&paths)?, keep: parse(&keep)?, default: Vec::new(), maps };
    Ok(Resolved { fields: FieldsDoc { records: output.records.to_string(), paths, keep, default: None }, declared, doc, root })
}

/// The `fields` the command document of `command` publishes for the output named `report`, when it declares one.
///
/// # Panics
/// When [`OUTPUTS`] is wrong for the report (`tests/atip_paging_fields.rs` finds it first).
pub fn doc_for(command: &str, report: &str) -> Option<FieldsDoc> {
    let output = OUTPUTS.iter().find(|output| output.command == command && output.report == report)?;
    Some(resolve(output).unwrap_or_else(|problem| panic!("the `fields` of `canon {command}`: {problem}")).fields)
}

/// What a run projects: the kept paths, and the schema its projected `data` is held to.
#[derive(Debug, Clone)]
pub struct Active {
    pub projection: Projection,
    pub doc: Value,
    pub root: Value,
}

static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

/// The projection this run applies to the `data` of its envelope, if `--fields` was given.
pub fn active() -> Option<Active> {
    ACTIVE.lock().expect("the projection slot is not poisoned").clone()
}

/// A valid `--fields` selection of `command`'s output, not yet applied to the envelope.
#[derive(Debug, Clone)]
pub struct Chosen {
    active: Active,
}

impl Chosen {
    /// The kept paths, each in its one spelling: what `meta.fields` lists.
    pub fn paths(&self) -> Vec<String> {
        self.active.projection.meta_fields()
    }

    /// Projects the records of `data` (a report as a JSON value), for a command that writes the report itself.
    pub fn apply(&self, data: &mut Value) {
        self.active.projection.apply(data);
    }

    /// Makes this the projection of the run's envelope: its `data` is projected and `meta.fields` lists the kept paths.
    pub fn activate(&self) {
        *ACTIVE.lock().expect("the projection slot is not poisoned") = Some(self.active.clone());
    }
}

/// Reads and checks `raw`, the value of `--fields` (`None` when it was not given), against the declaration of `command`.
/// `hint` adds what the caller knows about this call to the refusal's hint.
///
/// # Errors
/// `USAGE`, `field` `--fields`, `expected` the selectable paths, for a value that is empty, is not a path, or names
/// a path the command does not declare or always keeps.
pub fn choose(command: &str, raw: Option<&str>, hint: Option<&str>) -> Result<Option<Chosen>, CliError> {
    choose_output(command, None, raw, hint)
}

/// [`choose`] against the output named `report` of a command that has several (`export`): `--fields` is read against
/// the output the invocation selects (spec 6.10). `None` is the command's first output.
///
/// # Errors
/// As [`choose`].
pub fn choose_output(command: &str, report: Option<&str>, raw: Option<&str>, hint: Option<&str>) -> Result<Option<Chosen>, CliError> {
    let Some(output) = OUTPUTS.iter().find(|output| output.command == command && report.is_none_or(|report| output.report == report)) else { return Ok(None) };
    let resolved = resolve(output).unwrap_or_else(|problem| panic!("the `fields` of `canon {command}`: {problem}"));
    let selection: Option<Selection> = resolved.declared.select(raw).map_err(|error| refusal(command, &error, hint))?;
    Ok(selection.map(|selection| Chosen { active: Active { projection: resolved.declared.projection(&selection), doc: resolved.doc, root: resolved.root } }))
}

/// [`choose`], and the run's projection when `--fields` was given.
pub fn select(command: &str, raw: Option<&str>, hint: Option<&str>) -> Result<(), CliError> {
    if let Some(chosen) = choose(command, raw, hint)? {
        chosen.activate();
    }
    Ok(())
}

fn refusal(command: &str, error: &atip::fields::SelectError, hint: Option<&str>) -> CliError {
    let suggestions = crate::suggest::nearest(&error.received, error.expected.iter().map(String::as_str));
    let mut text = format!("--fields takes a comma list of the paths `canon schema {command} --json` declares under outputs[].fields.paths; a path is the record's own key, written as it is");
    if let Some(hint) = hint {
        text.push_str("; ");
        text.push_str(hint);
    }
    CliError::refused("USAGE", format!("`canon {command}`: {error}")).field("--fields").expected(error.expected.iter().cloned()).received(error.received.clone()).suggestions(suggestions).hint(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_output_resolves_and_keeps_its_kept_paths_out_of_the_selectable_ones() {
        for output in OUTPUTS {
            let resolved = resolve(output).unwrap_or_else(|problem| panic!("{problem}"));
            assert!(!resolved.fields.paths.is_empty(), "{}", output.command);
            for kept in &resolved.fields.keep {
                assert!(!resolved.fields.paths.contains(kept), "{}: {kept}", output.command);
            }
        }
    }

    #[test]
    fn a_path_in_the_old_case_style_is_refused_and_the_refusal_lists_the_declared_paths() {
        let refused = choose("finding list", Some("resolution_sha"), None).expect_err("snake_case is not a path of a report");
        assert_eq!(refused.code, "USAGE");
        assert_eq!(refused.field_name(), Some("--fields"));
        assert!(refused.expected_values().is_some_and(|expected| expected.iter().any(|path| path == "resolutionSha")));
        assert!(choose("finding list", Some("resolutionSha,summary"), None).expect("declared").is_some());
    }
}
