//! The contract repository's examples as test input (approaches/checker.md 7.2, the last paragraph).
//!
//! `tests/fixtures/atip-examples/` holds copies pinned to the tag v1.1.0 of https://github.com/journeyWorker/atip;
//! `TABLE.md` says what each file is expected to do. The valid documents pass every layer `atip check` applies to
//! one envelope (the envelope schema, `null`, the contract markers, the registry entries and what the document emits,
//! the hint form, the failure rules of `L0.ok-exit`), the ones that break one of those rules are rejected by the
//! layer that owns the rule, and the ones that break only an L1 or L2 rule are skipped with the reason in the
//! table. The layers are the functions the checks of a run call (`checks::rules`), not a copy of them.

mod support;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use atip_check::checks::rules::{
    contract_version_findings, entry_problems, envelope_problems, hint_problems, next_findings,
    null_problems, page_findings, reason_names, registry_findings,
};
use atip_check::checks::{failure_problems, unknown_command_problems};
use atip_check::claim::Claim;
use atip_check::fixture::RegistryRow;
use atip_check::schema::schemas;

/// The tag of the contract repository the copies and the pinned digests come from, and the commit it points at.
const PINNED: &str = "v1.1.0 (da069df042b1140915d6a5e674b306bf7b5cfd187)";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/atip-examples")
}

struct Row {
    file: String,
    expect: String,
    by: String,
    why: String,
}

fn table() -> Vec<Row> {
    let text = std::fs::read_to_string(root().join("TABLE.md")).expect("TABLE.md");
    text.lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            assert_eq!(cells.len(), 4, "{line}");
            Row {
                file: cells[0].trim_matches('`').to_string(),
                expect: cells[1].to_string(),
                by: cells[2].to_string(),
                why: cells[3].to_string(),
            }
        })
        .collect()
}

/// The envelope a file holds: the document of a `.json` file, the `result` of the `end` line of a `.ndjson` stream (or
/// its one line when that is an envelope); `None` when a stream has no end line to read one from.
fn load(file: &str) -> Option<Value> {
    let text = std::fs::read_to_string(root().join(file)).expect("example");
    if file.ends_with(".ndjson") {
        let lines: Vec<Value> = text.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).collect();
        // A usage error prints one plain envelope; a stream ends with its `end` line, whose `result` is the envelope.
        if let [only] = lines.as_slice() {
            return only.get("ok").is_some().then(|| only.clone());
        }
        return lines
            .iter()
            .filter(|line| line["type"] == "end")
            .find_map(|line| line.get("result").cloned());
    }
    Some(serde_json::from_str(&text).unwrap_or_else(|error| panic!("{file}: {error}")))
}

/// The registry of the tool a document comes from: the rows of its error registry, and the codes of its warning registry.
fn registry_of(tool: &str) -> Option<(Vec<RegistryRow>, Vec<String>)> {
    let (errors, warnings) = match tool {
        "acme" => ("schema-errors-acme.json", Some("schema-warnings-acme.json")),
        "canon" => ("schema-errors.json", None),
        "ledger" => ("schema-errors-ledger.json", None),
        _ => return None,
    };
    let rows = load(errors)?["data"]["errors"]
        .as_array()?
        .iter()
        .map(|row| RegistryRow {
            code: row["code"].as_str().expect("code").to_string(),
            exit: row["exit"].as_i64().expect("exit"),
            retryable: row["retryable"].as_bool().expect("retryable"),
            reasons: Some(reason_names(row)),
        })
        .collect();
    let warning_codes = warnings
        .and_then(load)
        .and_then(|document| document["data"]["warnings"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|row| row["code"].as_str().map(str::to_string))
        .collect();
    Some((rows, warning_codes))
}

/// The minor the tool of a document claims: the synthetic tool `acme` claims 1.1 (a document of it without the markers
/// is a 1.1 tool's ordinary answer that lacks them); another tool claims what its document declares, 1.1 when
/// `meta.contractVersion` is there and other than `"1.0"` (an unknown minor is a claim of 1.1 it does not keep), else 1.0.
fn claim_of(document: &Value) -> Claim {
    if document.pointer("/meta/tool").and_then(Value::as_str) == Some("acme") {
        return Claim::V1_1;
    }
    match document.pointer("/meta/contractVersion").and_then(Value::as_str) {
        Some("1.0") | None => Claim::V1_0,
        Some(_) => Claim::V1_1,
    }
}

/// What `atip check` finds wrong with one envelope, by the check that owns the rule.
fn layers(document: &Value) -> Vec<(&'static str, Vec<String>)> {
    let claim = claim_of(document);
    let mut found = vec![
        ("L0.envelope", envelope_problems(document, claim)),
        ("L0.no-null", null_problems(document)),
        (
            "L0.contract-version",
            contract_version_findings(document, claim)
                .into_iter()
                .filter_map(|(_, problem)| problem)
                .collect(),
        ),
    ];
    let tool = document.pointer("/meta/tool").and_then(Value::as_str).unwrap_or_default();
    if claim.is_1_1() {
        found.push(("L0.hint-form", hint_problems(document, tool)));
    }
    let registry = registry_of(tool);
    let mut registered = entry_problems(document, claim);
    if let Some((rows, warning_codes)) = &registry {
        registered.extend(
            registry_findings(document, rows, warning_codes, claim)
                .into_iter()
                .filter_map(|(_, problem)| problem),
        );
    }
    found.push(("L0.registry", registered));
    let collect = |findings: Vec<(&'static str, Option<String>)>| -> Vec<String> { findings.into_iter().filter_map(|(_, problem)| problem).collect() };
    found.push(("L0.next", collect(next_findings(document, claim, true, tool))));
    found.push(("L0.page", collect(page_findings(document, claim))));
    if document["ok"] == false {
        if let Some((rows, _)) = &registry {
            found.push(("L0.ok-exit", failure_problems(document, rows, None)));
        }
        if document["error"]["code"] == "UNKNOWN_COMMAND" {
            found.push(("L0.unknown-command", unknown_command_problems(document)));
        }
    }
    found
}

#[test]
fn atip_check_34_every_valid_example_of_the_contract_repository_passes_every_layer() {
    let mut accepted = 0;
    for row in table().iter().filter(|row| row.expect == "accept") {
        let document = load(&row.file).unwrap_or_else(|| panic!("{} has an envelope", row.file));
        for (layer, problems) in layers(&document) {
            assert!(
                problems.is_empty(),
                "{} is rejected by {layer}: {problems:?}",
                row.file
            );
        }
        accepted += 1;
    }
    assert!(accepted >= 40, "the report, registry and stream examples of {PINNED}: {accepted}");
    // The one that the unknown-command rules apply to is among them.
    assert_eq!(
        load("report-failure-unknown-command.json").expect("file")["error"]["code"],
        "UNKNOWN_COMMAND"
    );
}

#[test]
fn atip_check_35_every_invalid_example_is_rejected_by_its_owner_and_the_l1_and_l2_only_ones_are_skipped_with_a_reason(
) {
    let table = table();
    for row in &table {
        match row.expect.as_str() {
            "reject" => {
                let document = load(&row.file).unwrap_or_else(|| panic!("{} has an envelope", row.file));
                let found = layers(&document);
                let by = found
                    .iter()
                    .find(|(layer, _)| *layer == row.by)
                    .unwrap_or_else(|| panic!("{}: {} did not look at it", row.file, row.by));
                assert!(
                    !by.1.is_empty(),
                    "{} must be rejected by {}: {found:?}",
                    row.file,
                    row.by
                );
                // One owner per pass condition: no other layer rejects it.
                let others: Vec<&str> = found
                    .iter()
                    .filter(|(layer, problems)| *layer != row.by && !problems.is_empty())
                    .map(|(layer, _)| *layer)
                    .collect();
                assert!(others.is_empty(), "{} is rejected by {} and also by {others:?}", row.file, row.by);
            }
            "skip" => {
                assert!(
                    row.by.starts_with("L1.") || row.by.starts_with("L2."),
                    "{}: a skip names the L1 or L2 check that owns the rule: {}",
                    row.file,
                    row.by
                );
                assert!(row.why.len() > 10, "{}: a skip says why", row.file);
                if let Some(document) = load(&row.file) {
                    for (layer, problems) in layers(&document) {
                        assert!(
                            problems.is_empty(),
                            "{} is skipped as an {} matter but {layer} rejects it: {problems:?}",
                            row.file,
                            row.by
                        );
                    }
                }
            }
            "accept" => {}
            other => panic!("{}: unknown expectation {other}", row.file),
        }
    }
    let rejected = table.iter().filter(|row| row.expect == "reject").count();
    assert!(rejected >= 20, "the L0 matters among the invalid examples: {rejected}");
    // Every file under the copies has a row, and every row a file.
    let mut on_disk: Vec<String> = Vec::new();
    for dir in ["", "invalid"] {
        for entry in std::fs::read_dir(root().join(dir)).expect("dir") {
            let path = entry.expect("entry").path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json" || extension == "ndjson")
            {
                on_disk.push(
                    path.strip_prefix(root())
                        .expect("prefix")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    on_disk.sort();
    let mut tabled: Vec<String> = table.iter().map(|row| row.file.clone()).collect();
    tabled.sort();
    assert_eq!(on_disk, tabled, "the table and the copied files agree");
}

#[test]
fn atip_check_35_a_failure_envelope_that_breaks_the_registry_is_rejected_by_the_rule_that_names_it() {
    let rows = vec![RegistryRow {
        code: "FINDING_NOT_FOUND".into(),
        exit: 2,
        retryable: false,
        reasons: None,
    }];
    let failure = |extra: Value| {
        let mut document = json!({"ok": false, "error": {"code": "FINDING_NOT_FOUND", "message": "m", "retryable": false}, "meta": {"schemaVersion": "1.0", "tool": "t", "toolVersion": "1"}});
        for (key, value) in extra.as_object().expect("object") {
            document["error"][key] = value.clone();
        }
        document
    };
    assert!(failure_problems(&failure(json!({})), &rows, Some(2)).is_empty());
    assert!(
        failure_problems(&failure(json!({"retryable": true})), &rows, Some(2))[0]
            .contains("`retryable` true, the registry says false")
    );
    assert!(
        failure_problems(&failure(json!({"code": "NOPE"})), &rows, Some(2)).is_empty(),
        "a code the registry does not list is L0.registry's, not L0.ok-exit's"
    );
    let findings = registry_findings(&failure(json!({"code": "NOPE"})), &rows, &[], Claim::V1_0);
    assert!(findings.iter().any(|(id, problem)| *id == "emitted-codes-registered" && problem.is_some()));
    assert!(failure_problems(&failure(json!({})), &rows, Some(1))[0]
        .contains("exited 1, the registry says 2"));
    let mut with_data = failure(json!({}));
    with_data["data"] = json!({});
    assert!(failure_problems(&with_data, &rows, Some(2))[0].contains("carries `data`"));
    assert!(
        failure_problems(&with_data, &rows, None)[0].contains("carries `data`"),
        "without an exit the registered exit stands for it"
    );
}

#[test]
fn atip_check_38_the_embedded_contract_schemas_are_the_originals_and_a_format_keyword_is_an_annotation(
) {
    // The schemas the checker validates against are byte for byte the pinned originals (sha256 of `schema/*.schema.json` at v1.1.0).
    for (name, text, digest) in [
        (
            "envelope.schema.json",
            atip::schema::ENVELOPE_SCHEMA,
            "d8b42e992a7750f857b7b9286925645ee4401ead264e3d27ff62db382a902a37",
        ),
        (
            "error.schema.json",
            atip::schema::ERROR_SCHEMA,
            "b89419efd8731950ac458a59e3631b50cf29f0168c141e087a2dfa18a15d4903",
        ),
        (
            "registry-entry.schema.json",
            atip::schema::REGISTRY_ENTRY_SCHEMA,
            "18e73eec11f662d6c19e18574a1c1d3424f037f0ad5d232f24ab0a56fab28622",
        ),
        (
            "warning-entry.schema.json",
            atip::schema::WARNING_ENTRY_SCHEMA,
            "66d2eaaea38eaf723343c262dea4b710bcd1b22dc547124f68f0abc2ee0fcfde",
        ),
        (
            "stream-line.schema.json",
            atip::schema::STREAM_LINE_SCHEMA,
            "cfa0647e4907a9447a992be8f9226b5873dc9b3cae89b5a9073c5fafef75c145",
        ),
    ] {
        let actual: String = Sha256::digest(text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            actual, digest,
            "{name} is not the original of {PINNED}"
        );
    }
    // A null where the contract forbids one is rejected by the schema; `L0.no-null` names it, and `L0.envelope` leaves it.
    let null_hint = json!({"ok": false, "error": {"code": "USAGE", "message": "m", "retryable": false, "hint": null}, "meta": {"schemaVersion": "1.0", "tool": "t", "toolVersion": "1"}});
    assert!(!schemas().envelope_problems(&null_hint).is_empty());
    assert!(envelope_problems(&null_hint, Claim::V1_0).is_empty());
    assert_eq!(null_problems(&null_hint), ["/error/hint is null"]);
    let null_meta = json!({"ok": true, "data": {}, "meta": {"schemaVersion": "1.0", "tool": "t", "toolVersion": "1", "command": null}});
    assert!(!schemas().envelope_problems(&null_meta).is_empty());
    // `format` is an annotation: a value that is not a date-time passes a schema that says it is one.
    let validator = schemas().validator(&json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "string", "format": "date-time"})).expect("compiles");
    assert!(validator.is_valid(&json!("not a date")));
    assert!(
        !validator.is_valid(&json!(3)),
        "the other keywords still assert"
    );
}

/// Prints the classification of every copied file: the layers that reject it. Run by hand to draft `TABLE.md`.
#[test]
#[ignore = "drafts TABLE.md by hand"]
fn draft_the_table() {
    let mut files: Vec<String> = Vec::new();
    for dir in ["", "invalid"] {
        for entry in std::fs::read_dir(root().join(dir)).expect("dir") {
            let path = entry.expect("entry").path();
            if path.is_file() && path.extension().is_some_and(|e| e == "json" || e == "ndjson") {
                files.push(path.strip_prefix(root()).expect("prefix").to_string_lossy().into_owned());
            }
        }
    }
    files.sort();
    for file in files {
        let by: Vec<&str> = load(&file)
            .map(|document| layers(&document).into_iter().filter(|(_, problems)| !problems.is_empty()).map(|(layer, _)| layer).collect())
            .unwrap_or_default();
        println!("DRAFT {file} {}", by.join(","));
    }
}
