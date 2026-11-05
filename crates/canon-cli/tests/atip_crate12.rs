//! canon-0-35-crate-1-2, seen from canon: the `atip` crate moves to 1.2; canon declares its minor by a constant of its own (1.2 in `canon-0-35-claim-l2`, 1.4 in `canon-0-36-claim-1-4`).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn canon(dir: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").output().expect("spawn canon");
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{args:?}: {error}: {}", String::from_utf8_lossy(&output.stdout)))
}

/// The Rust sources under `dir`, with comment lines left out.
fn sources_without_comments(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).expect("a source directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).expect("a source file");
                let code: String = text.lines().filter(|line| !line.trim_start().starts_with("//")).map(|line| format!("{line}\n")).collect();
                found.push((path, code));
            }
        }
    }
    found
}

#[test]
fn atip_crate12_12_canon_declares_the_minor_of_its_answers_by_its_own_constant_and_the_crate_knows_it() {
    assert!(atip::Minor::parse(canon_cli::json_out::DECLARED_MINOR).is_ok(), "the crate knows the minor canon declares");
    assert_eq!(canon_cli::json_out::DECLARED_MINOR, "1.4", "canon declares 1.4: its own constant, changed by a decision to claim the minor");
    let dir = tempfile::tempdir().expect("a temp dir");
    let schema = canon(dir.path(), &["schema", "--json"]);
    assert_eq!(schema["meta"]["contractVersion"], "1.4", "{schema}");
    assert_eq!(schema["meta"]["schemaVersion"], "1.4", "the self-description follows the declared minor");
    // A refusal declares it too: `status` finds no canon.yaml here.
    let status = canon(dir.path(), &["status", "--json"]);
    assert_eq!((status["ok"].clone(), status["meta"]["contractVersion"].clone()), (Value::Bool(false), Value::from("1.4")), "{status}");
    assert!(atip::envelope_problems(&status).is_empty(), "a 1.4 document is valid against the vendored schema");
}

#[test]
fn atip_crate12_13_a_text_that_is_no_minor_is_refused_and_no_source_of_canon_reads_the_crates_contract_version() {
    for text in ["1.9", "1.6", "2.0", "1", "", "1.1.0", "v1.1"] {
        assert_eq!(atip::Minor::parse(text), Err(atip::UnknownMinor(text.to_string())), "`{text}` is no minor the crate knows, and is not read as the nearest one");
    }
    for text in ["1.0", "1.1", "1.2", "1.3", "1.4", "1.5"] {
        assert_eq!(atip::Minor::parse(text).map(atip::Minor::as_str), Ok(text), "the crate knows {text}");
    }
    let sources = sources_without_comments(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    assert!(sources.len() > 50, "the walk found canon's sources");
    let readers: Vec<&PathBuf> = sources.iter().filter(|(_, code)| code.contains("CONTRACT_VERSION")).map(|(path, _)| path).collect();
    assert!(readers.is_empty(), "canon declares its own minor (`json_out::DECLARED_MINOR`); the crate's CONTRACT_VERSION is read in {readers:?}");
}

/// atip.crate12.14: the reserved codes canon registers, as `canon schema --errors` prints them, have the exit and retryability of the crate's table
/// for the minor canon declares, and the crate's own check finds nothing to say about any of them.
#[test]
fn atip_crate12_14_the_reserved_codes_canon_registers_have_the_exit_and_retryability_of_the_crates_table() {
    let minor = atip::Minor::parse(canon_cli::json_out::DECLARED_MINOR).expect("the crate knows the declared minor");
    let dir = tempfile::tempdir().expect("a temp dir");
    let registry = canon(dir.path(), &["schema", "--errors", "--json"]);
    let rows: std::collections::BTreeMap<&str, (i64, bool)> = registry["data"]["errors"]
        .as_array()
        .expect("the registry")
        .iter()
        .map(|row| (row["code"].as_str().expect("a code"), (row["exit"].as_i64().expect("an exit"), row["retryable"].as_bool().expect("retryable"))))
        .collect();
    let reserved: Vec<&atip::ReservedCode> = atip::reserved_codes(minor).collect();
    assert_eq!(reserved.len(), 12, "the twelve core codes at {minor:?}: STOPPED_MIDWAY is reserved only in a 1.2 document");
    let mut registered = 0;
    for code in &reserved {
        match rows.get(code.name) {
            Some((exit, retryable)) => {
                registered += 1;
                assert_eq!((*exit, *retryable), (i64::from(code.exit), code.retryable), "{}: canon registers it as the table has it", code.name);
                assert_eq!(atip::registration_problem(minor, code.name, Some(*exit), Some(*retryable)), None, "{}", code.name);
            }
            None => assert_eq!(code.name, "OPERATION_REUSED", "the one reserved code canon has no use for: it has no --operation"),
        }
    }
    assert_eq!(registered, 11);
    // The table is the crate's, in one place: thirteen rows, the last one reserved in 1.2 alone, at exit 1 and not retryable.
    assert_eq!(atip::RESERVED_CODES.len(), 13);
    let stopped = atip::RESERVED_CODES.iter().find(|code| code.name == "STOPPED_MIDWAY").expect("a row");
    assert_eq!((stopped.exit, stopped.retryable, stopped.since, stopped.until), (1, false, atip::Minor::V1_2, Some(atip::Minor::V1_2)));
    assert_eq!(atip::reserved_codes(atip::Minor::V1_2).count(), 13);
    assert!(!rows.contains_key("STOPPED_MIDWAY"), "canon registers no code of that name");
}

/// atip.crate12.15: a registry that registers a reserved code at another exit or retryability is found by the crate's check, and `STOPPED_MIDWAY`
/// is judged only in a document that declares 1.2.
#[test]
fn atip_crate12_15_a_reserved_code_registered_with_another_exit_or_retryability_is_a_problem_and_the_midway_code_is_judged_in_1_2_alone() {
    let minor = atip::Minor::parse(canon_cli::json_out::DECLARED_MINOR).expect("the crate knows the declared minor");
    let mut judged = 0;
    for row in canon_cli::cli_error::all_codes() {
        let Some(reserved) = atip::reserved_code(row.code, minor) else { continue };
        judged += 1;
        assert_eq!(atip::registration_problem(minor, row.code, Some(i64::from(row.exit)), Some(row.retryable)), None, "{}", row.code);
        let other_exit = if reserved.exit == 1 { 2 } else { 1 };
        let problem = atip::registration_problem(minor, row.code, Some(other_exit), Some(reserved.retryable)).unwrap_or_else(|| panic!("{} at exit {other_exit} is a problem", row.code));
        assert!(problem.contains(row.code) && problem.contains(&format!("reserved is exit {}", reserved.exit)), "{problem}");
        assert!(atip::registration_problem(minor, row.code, Some(i64::from(reserved.exit)), Some(!reserved.retryable)).is_some(), "{}: retryable flipped", row.code);
        assert!(atip::registration_problem(minor, row.code, None, Some(reserved.retryable)).is_some(), "{}: a row that states no exit is not read as right", row.code);
        assert!(atip::registration_problem(minor, row.code, Some(i64::from(reserved.exit)), None).is_some(), "{}: nor one that states no retryable", row.code);
    }
    assert_eq!(judged, 11, "the reserved codes canon registers");
    // A registry that registers STOPPED_MIDWAY at exit 2: a problem in a 1.2 document (the code is reserved at exit 1), none in a 1.1 one (the code is the
    // tool's own) and none in canon's own 1.4 one (it is no code at all, since it ends in no condition).
    let at_exit_two = |minor: atip::Minor| atip::registration_problem(minor, "STOPPED_MIDWAY", Some(2), Some(false));
    assert!(at_exit_two(atip::Minor::V1_2).is_some_and(|problem| problem.contains("reserved is exit 1")));
    assert_eq!(at_exit_two(atip::Minor::V1_1), None);
    assert_eq!(at_exit_two(minor), None);
    assert!(canon_cli::cli_error::code_info("STOPPED_MIDWAY").is_none(), "and canon registers none");
    let dir = tempfile::tempdir().expect("a temp dir");
    let refused = canon(dir.path(), &["schema", "--error", "STOPPED_MIDWAY", "--json"]);
    assert_eq!((refused["error"]["code"].as_str(), refused["error"]["field"].as_str()), (Some("USAGE"), Some("--error")), "{refused}");
}

/// atip.crate12.18: the rules a `fields` declaration is held to are those of the minor canon declares, and a minor the crate does not know is refused.
#[test]
fn atip_crate12_18_field_rules_follow_the_declared_minor_and_a_minor_the_crate_does_not_know_is_refused() {
    use atip::fields::{check_declaration, Rules};
    let declared = Rules::for_minor(canon_cli::json_out::DECLARED_MINOR).expect("canon declares a minor the crate knows");
    assert_eq!(declared, Rules::at(atip::Minor::V1_4));
    assert_eq!(declared, Rules::for_minor("1.1").expect("a minor"), "1.1 to 1.4 keep the same rules");
    assert_ne!(declared, Rules::for_minor("1.0").expect("a minor"), "1.0 has no bracketed key, no map key and no map cut node");
    for text in ["1.9", "1.6", "2.0", "", "1"] {
        assert_eq!(Rules::for_minor(text), Err(atip::UnknownMinor(text.to_string())), "`{text}` is refused, not read as the nearest minor");
    }
    // What the rules change: a key that is not an identifier is written in brackets, which canon's declared rules allow and 1.0's refuse.
    let doc = json!({ "schemas": { "Report": { "type": "object", "properties": { "rows": { "type": "array", "items": { "$ref": "#/schemas/Row" } } } }, "Row": { "type": "object", "properties": { "attributes": { "type": "object", "additionalProperties": { "type": "string" } } } } } });
    let bracketed = |rules: Rules| check_declaration(&doc, &json!({ "$ref": "#/schemas/Report" }), "/rows", &["attributes".to_string(), "attributes[\"x.y\"]".to_string()], &[], &[], rules);
    assert_eq!(bracketed(declared), Vec::<String>::new());
    assert!(bracketed(Rules::for_minor("1.0").expect("a minor")).iter().any(|problem| problem.contains("bracketed key")), "{:?}", bracketed(Rules::for_minor("1.0").expect("a minor")));
    // Every declaration canon publishes keeps the contract under the rules of its minor.
    let mut declarations = 0;
    for command in canon_cli::spec::model()["commands"].as_array().expect("the commands") {
        for output in command["outputs"].as_array().into_iter().flatten().filter(|output| output.get("fields").is_some()) {
            declarations += 1;
            let strings = |key: &str| -> Vec<String> { output["fields"][key].as_array().into_iter().flatten().map(|path| path.as_str().expect("a path").to_string()).collect() };
            let schemas = command["schemas"].as_object().expect("the schemas");
            let problems = check_declaration(&atip::fields::schemas_document(schemas), &json!({ "$ref": output["schemaRef"] }), output["fields"]["records"].as_str().expect("records"), &strings("paths"), &strings("keep"), &strings("default"), declared);
            assert_eq!(problems, Vec::<String>::new(), "{}", command["command"]);
        }
    }
    assert_eq!(declarations, 8, "the six commands that project, export with three outputs");
    // Canon builds its rules by the declared minor and never by the crate's field for the old behaviour.
    let sources = sources_without_comments(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    let by_field: Vec<&PathBuf> = sources.iter().filter(|(_, code)| code.contains("v11")).map(|(path, _)| path).collect();
    assert!(by_field.is_empty(), "no source builds Rules by its old field: {by_field:?}");
}
