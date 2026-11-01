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

/// Schemas of a command document that none of its refs names, and why. None: a document holds only what its refs name.
const UNLISTED_SCHEMAS: &[(&str, &str)] = &[];

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
        let reports: BTreeSet<String> = outputs.iter().map(report_of).collect();
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
            let name = report_of(output);
            let name = name.as_str();
            let version = output["schemaVersion"].as_str().unwrap_or_default();
            let well_formed = version.split_once('.').is_some_and(|(major, minor)| !major.is_empty() && !minor.is_empty() && major.chars().chain(minor.chars()).all(|c| c.is_ascii_digit()));
            if !well_formed {
                problems.push(format!("`{path}`: {name} has schemaVersion {version:?}, not <major>.<minor>"));
            }
            if node["schemas"].get(name).is_none() {
                problems.push(format!("`{path}`: {name} names no schema of the spec"));
            }
            // The report is named for its leaf: `<PascalCaseId>Report`, or a variant of it (`Batch…`, a second shape).
            let pascal: String = path.split([' ', '-']).map(|w| w[..1].to_uppercase() + &w[1..]).collect();
            if !name.ends_with("Report") || !(name.contains(&pascal) || name.starts_with("Batch")) {
                problems.push(format!("`{path}`: {name} is not named for its leaf (`{pascal}Report`)"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Every schema a command document holds is named by a ref of its outputs or its input, or by another schema of the document.
#[test]
fn no_schema_of_a_command_document_is_orphaned() {
    fn refs_in(value: &serde_json::Value, out: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    match (key.as_str(), child.as_str().and_then(|text| text.strip_prefix("#/schemas/"))) {
                        ("$ref", Some(name)) => {
                            out.insert(name.to_string());
                        }
                        _ => refs_in(child, out),
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    refs_in(item, out);
                }
            }
            _ => {}
        }
    }
    let mut problems = Vec::new();
    let mut checked = 0;
    for node in leaves() {
        let mut listed: BTreeSet<String> = node["outputs"].as_array().unwrap().iter().map(report_of).collect();
        listed.extend(node["input"].as_object().into_iter().flat_map(|input| ["schemaRef", "argumentsRef"].into_iter().filter_map(|key| input.get(key)?.as_str()?.strip_prefix("#/schemas/").map(str::to_string))));
        // What a listed schema refers to is declared with it.
        let mut queue: Vec<String> = listed.iter().cloned().collect();
        while let Some(name) = queue.pop() {
            let mut inner = BTreeSet::new();
            if let Some(schema) = node["schemas"].get(&name) {
                refs_in(schema, &mut inner);
            }
            for found in inner {
                if listed.insert(found.clone()) {
                    queue.push(found);
                }
            }
        }
        for name in node["schemas"].as_object().expect("schemas").keys() {
            checked += 1;
            match (listed.contains(name.as_str()), UNLISTED_SCHEMAS.iter().find(|(n, _)| n == name)) {
                (false, None) => problems.push(format!("`{}`: the schema {name} is in the document and no ref reaches it", path_of(node))),
                (true, Some(_)) => problems.push(format!("UNLISTED_SCHEMAS names {name}, which a ref of `{}` now reaches: drop it from the list", path_of(node))),
                _ => {}
            }
        }
        for listed_name in &listed {
            if node["schemas"].get(listed_name).is_none() {
                problems.push(format!("`{}`: a ref names {listed_name}, which the document does not hold", path_of(node)));
            }
        }
    }
    for (name, reason) in UNLISTED_SCHEMAS {
        assert!(!reason.is_empty(), "{name}");
    }
    assert!(checked > 200, "{checked} schemas were checked");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_declared_report_has_a_report_schema_row_and_every_row_a_spec_schema() {
    let rows = canon_cli::reports::report_schemas();
    let mut problems = Vec::new();
    for row in &rows {
        // A report no command prints (`DashboardStatusReport` is served over HTTP) is in no document.
        let printed = leaves().iter().any(|n| path_of(n) == row.command && n["schemas"].get(&row.name).is_some());
        let served = ["DashboardStatusReport", "DashboardBriefsReport"].contains(&row.name.as_str());
        if !printed && !served && !row.name.starts_with("Batch") {
            problems.push(format!("report_schemas() has {} (`{}`) and the command's document has no such schema", row.name, row.command));
        }
        if !leaves().iter().any(|n| path_of(n) == row.command) {
            problems.push(format!("report_schemas() row {} names `{}`, which is no leaf", row.name, row.command));
        }
    }
    for node in leaves() {
        for output in node["outputs"].as_array().unwrap() {
            let name = report_of(output);
            if !rows.iter().any(|r| r.name == name && r.command == path_of(node)) {
                problems.push(format!("`{}` outputs {name}, which has no report_schemas() row for it", path_of(node)));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
