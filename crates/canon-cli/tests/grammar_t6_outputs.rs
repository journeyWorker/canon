//! T6 (0.28 D2): every leaf declares what it prints, and every schema the spec
//! publishes is printed by some leaf.
//!
//! An output is a `report`, an `artifact` (bytes without `--json`, enveloped with it)
//! or a `stream`; each names a schema of the spec and its version. A leaf with no
//! output would need a stated reason, and none has one ([`NO_OUTPUT`]).

mod harness;

use std::collections::BTreeSet;

use harness::*;

/// Leaves that declare `outputs: []`, and why. None: every leaf prints an enveloped report.
const NO_OUTPUT: &[(&str, &str)] = &[];

/// Schemas of the spec that no leaf lists in `outputs`, and why.
const UNLISTED_SCHEMAS: &[(&str, &str)] = &[
    ("DashboardStatusReport", "served over HTTP by `dashboard` (GET /api/status), never printed by a command"),
    ("DashboardBriefsReport", "served over HTTP by `dashboard` (GET /api/briefs), never printed by a command"),
];

#[test]
fn every_leaf_declares_its_outputs_with_a_schema_and_a_version() {
    let mut problems = Vec::new();
    for node in leaves() {
        let path = path_of(node);
        let outputs = node["outputs"].as_array().expect("outputs");
        if outputs.is_empty() {
            if !NO_OUTPUT.iter().any(|(leaf, reason)| *leaf == path && !reason.is_empty()) {
                problems.push(format!("`{path}` declares no output and has no reason in NO_OUTPUT"));
            }
            continue;
        }
        let reports: BTreeSet<&str> = outputs.iter().filter_map(|o| o["report"].as_str()).collect();
        if reports.len() != outputs.len() {
            problems.push(format!("`{path}` lists one report twice"));
        }
        for output in outputs {
            let kind = output["kind"].as_str().unwrap_or_default();
            if !["report", "artifact", "stream"].contains(&kind) {
                problems.push(format!("`{path}`: output kind {kind:?}"));
            }
            if kind == "artifact" && !output["mediaType"].as_str().is_some_and(|m| m.contains('/')) {
                problems.push(format!("`{path}`: an artifact without a media type"));
            }
            let name = output["report"].as_str().unwrap_or_default();
            let version = output["schemaVersion"].as_str().unwrap_or_default();
            let well_formed = version.split_once('.').is_some_and(|(major, minor)| !major.is_empty() && !minor.is_empty() && major.chars().chain(minor.chars()).all(|c| c.is_ascii_digit()));
            if !well_formed {
                problems.push(format!("`{path}`: {name} has schemaVersion {version:?}, not <major>.<minor>"));
            }
            if output["schemaRef"] != format!("#/schemas/{name}") || spec()["schemas"].get(name).is_none() {
                problems.push(format!("`{path}`: {name} names no schema of the spec"));
            }
            // The report is named for its leaf: `<PascalCaseId>Report`, or a variant of it (`Batch…`, a second shape).
            let pascal: String = path.split(|c: char| c == ' ' || c == '-').map(|w| w[..1].to_uppercase() + &w[1..]).collect();
            if !name.ends_with("Report") || !(name.contains(&pascal) || name.starts_with("Batch")) {
                problems.push(format!("`{path}`: {name} is not named for its leaf (`{pascal}Report`)"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn no_schema_of_the_spec_is_orphaned() {
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    for node in leaves() {
        listed.extend(node["outputs"].as_array().unwrap().iter().filter_map(|o| o["report"].as_str()));
        // A schema a leaf takes as `--input` is as declared as one it prints.
        listed.extend(node["inputs"].as_array().unwrap().iter().filter_map(|i| i["schemaRef"].as_str()).filter_map(|r| r.strip_prefix("#/schemas/")));
    }
    let mut problems = Vec::new();
    for name in spec()["schemas"].as_object().expect("schemas").keys() {
        match (listed.contains(name.as_str()), UNLISTED_SCHEMAS.iter().find(|(n, _)| n == name)) {
            (false, None) => problems.push(format!("schema {name} is in the spec and no leaf outputs it")),
            (true, Some(_)) => problems.push(format!("UNLISTED_SCHEMAS names {name}, which a leaf now outputs: drop it from the list")),
            _ => {}
        }
    }
    for (name, reason) in UNLISTED_SCHEMAS.iter() {
        assert!(!reason.is_empty());
        if spec()["schemas"].get(name).is_none() {
            problems.push(format!("UNLISTED_SCHEMAS names {name}, which is no schema: drop it from the list"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_declared_report_has_a_report_schema_row_and_every_row_a_spec_schema() {
    let rows = canon_cli::reports::report_schemas();
    let mut problems = Vec::new();
    for row in &rows {
        if spec()["schemas"].get(&row.name).is_none() {
            problems.push(format!("report_schemas() has {} (`{}`) and the spec has no such schema", row.name, row.command));
        }
        if !leaves().iter().any(|n| path_of(n) == row.command) {
            problems.push(format!("report_schemas() row {} names `{}`, which is no leaf", row.name, row.command));
        }
    }
    for node in leaves() {
        for output in node["outputs"].as_array().unwrap() {
            let name = output["report"].as_str().unwrap();
            if !rows.iter().any(|r| r.name == name && r.command == path_of(node)) {
                problems.push(format!("`{}` outputs {name}, which has no report_schemas() row for it", path_of(node)));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
