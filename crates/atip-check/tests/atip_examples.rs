//! The contract repository's examples as test input (approaches/checker.md 7.2, the last paragraph).
//!
//! `tests/fixtures/atip-examples/` holds copies pinned to commit 365244c of https://github.com/journeyWorker/atip;
//! `TABLE.md` says what each file is expected to do. The valid envelopes pass the layers `atip check` applies to one
//! envelope, the ones that break an envelope or error rule (L0) are rejected, and the ones that break only an L1 or
//! L2 rule are skipped with the reason in the table.

mod support;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use atip_check::checks::{failure_problems, unknown_command_problems};
use atip_check::fixture::RegistryRow;
use atip_check::schema::schemas;

/// The commit of the contract repository the copies and the pinned digests come from.
const PINNED: &str = "365244cb15ce55ea737dec6ca3d8b714441d899d";

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
            Row { file: cells[0].trim_matches('`').to_string(), expect: cells[1].to_string(), by: cells[2].to_string(), why: cells[3].to_string() }
        })
        .collect()
}

fn load(file: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join(file)).expect("example")).unwrap_or_else(|error| panic!("{file}: {error}"))
}

/// The registry rows of the two registries the examples ship.
fn registry() -> Vec<RegistryRow> {
    let mut rows: Vec<RegistryRow> = Vec::new();
    for file in ["schema-errors.json", "schema-errors-acme.json"] {
        for row in load(file)["data"]["errors"].as_array().expect("errors") {
            let code = row["code"].as_str().expect("code").to_string();
            if !rows.iter().any(|known| known.code == code) {
                rows.push(RegistryRow { code, exit: row["exit"].as_i64().expect("exit"), retryable: row["retryable"].as_bool().expect("retryable") });
            }
        }
    }
    rows
}

/// What `atip check` finds wrong with one envelope: the schema, the failure rules of L0.ok-exit, the rules of L0.unknown-command.
fn layers(document: &Value, rows: &[RegistryRow]) -> Vec<(&'static str, Vec<String>)> {
    let mut found = vec![("schema", schemas().envelope_problems(document))];
    if document["ok"] == false {
        found.push(("L0.ok-exit", failure_problems(document, rows, None)));
        if document["error"]["code"] == "UNKNOWN_COMMAND" {
            found.push(("L0.unknown-command", unknown_command_problems(document)));
        }
    }
    found
}

#[test]
fn atip_check_34_every_valid_envelope_example_of_the_contract_repository_passes() {
    let rows = registry();
    let mut accepted = 0;
    for row in table().iter().filter(|row| row.expect == "accept") {
        let document = load(&row.file);
        for (layer, problems) in layers(&document, &rows) {
            assert!(problems.is_empty(), "{} is rejected by {layer}: {problems:?}", row.file);
        }
        accepted += 1;
    }
    assert_eq!(accepted, 23, "the 23 report-*.json examples of the pinned commit");
    // The one that the unknown-command rules apply to is among them.
    assert_eq!(load("report-failure-unknown-command.json")["error"]["code"], "UNKNOWN_COMMAND");
}

#[test]
fn atip_check_35_every_invalid_envelope_example_is_rejected_and_the_l1_and_l2_only_ones_are_skipped_with_a_reason() {
    let rows = registry();
    let table = table();
    for row in &table {
        match row.expect.as_str() {
            "reject" => {
                let found = layers(&load(&row.file), &rows);
                let by = found.iter().find(|(layer, _)| *layer == row.by).unwrap_or_else(|| panic!("{}: {} did not look at it", row.file, row.by));
                assert!(!by.1.is_empty(), "{} must be rejected by {}: {found:?}", row.file, row.by);
            }
            "skip" => {
                assert!(row.by.starts_with("L1.") || row.by.starts_with("L2."), "{}: a skip names the L1 or L2 check that owns the rule: {}", row.file, row.by);
                assert!(row.why.len() > 10, "{}: a skip says why", row.file);
                for (layer, problems) in layers(&load(&row.file), &rows) {
                    assert!(problems.is_empty(), "{} is skipped as an {} matter but {layer} rejects it: {problems:?}", row.file, row.by);
                }
            }
            "accept" | "registry" => {}
            other => panic!("{}: unknown expectation {other}", row.file),
        }
    }
    let rejected = table.iter().filter(|row| row.expect == "reject").count();
    let skipped = table.iter().filter(|row| row.expect == "skip").count();
    assert_eq!((rejected, skipped), (7 + 3, 24), "7 envelope-*.json and 3 report-*.json are L0 matters; the other 24 report-*.json are L1 and L2 matters");
    // Every file under the copies has a row, and every row a file.
    let mut on_disk: Vec<String> = Vec::new();
    for dir in ["", "invalid"] {
        for entry in std::fs::read_dir(root().join(dir)).expect("dir") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|extension| extension == "json") {
                on_disk.push(path.strip_prefix(root()).expect("prefix").to_string_lossy().into_owned());
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
    let rows = vec![RegistryRow { code: "FINDING_NOT_FOUND".into(), exit: 2, retryable: false }];
    let failure = |extra: Value| {
        let mut document = json!({"ok": false, "error": {"code": "FINDING_NOT_FOUND", "message": "m", "retryable": false}, "meta": {"schemaVersion": "1.0", "tool": "t", "toolVersion": "1"}});
        for (key, value) in extra.as_object().expect("object") {
            document["error"][key] = value.clone();
        }
        document
    };
    assert!(failure_problems(&failure(json!({})), &rows, Some(2)).is_empty());
    assert!(failure_problems(&failure(json!({"retryable": true})), &rows, Some(2))[0].contains("`retryable` true, the registry says false"));
    assert!(failure_problems(&failure(json!({"code": "NOPE"})), &rows, Some(2))[0].contains("not in the registry"));
    assert!(failure_problems(&failure(json!({})), &rows, Some(1))[0].contains("exited 1, the registry says 2"));
    let mut with_data = failure(json!({}));
    with_data["data"] = json!({});
    assert!(failure_problems(&with_data, &rows, Some(2))[0].contains("carries `data`"));
    assert!(failure_problems(&with_data, &rows, None)[0].contains("carries `data`"), "without an exit the registered exit stands for it");
}

#[test]
fn atip_check_38_the_embedded_contract_schemas_are_the_originals_and_a_format_keyword_is_an_annotation() {
    // The three schemas the checker validates against are byte for byte the pinned originals (sha256 of `schema/*.schema.json` at 365244c).
    for (name, text, digest) in [
        ("envelope.schema.json", atip::schema::ENVELOPE_SCHEMA, "055ace2ffb10eddba2c62fdff63155a83be4a8c1ba6ece73f6922f561ac8b299"),
        ("error.schema.json", atip::schema::ERROR_SCHEMA, "49afaa3bf93d848b1e477b19b3969680b4a0818e46dbdc6c54db248d4ecfdcfb"),
        ("registry-entry.schema.json", atip::schema::REGISTRY_ENTRY_SCHEMA, "411228ac4a2cc79ef4079161e9d015c5352bca49953aa33bf7007d0b100daedc"),
    ] {
        let actual: String = Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(actual, digest, "{name} is not the original of commit {PINNED}");
    }
    // A null where the contract forbids one is rejected: the schema is what judges `L0.no-null`.
    let null_hint = json!({"ok": false, "error": {"code": "USAGE", "message": "m", "retryable": false, "hint": null}, "meta": {"schemaVersion": "1.0", "tool": "t", "toolVersion": "1"}});
    assert!(!schemas().envelope_problems(&null_hint).is_empty());
    let null_meta = json!({"ok": true, "data": {}, "meta": {"schemaVersion": "1.0", "tool": "t", "toolVersion": "1", "command": null}});
    assert!(!schemas().envelope_problems(&null_meta).is_empty());
    // `format` is an annotation: a value that is not a date-time passes a schema that says it is one.
    let validator = schemas().validator(&json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "string", "format": "date-time"})).expect("compiles");
    assert!(validator.is_valid(&json!("not a date")));
    assert!(!validator.is_valid(&json!(3)), "the other keywords still assert");
}
