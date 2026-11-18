//! `canon-0-35-crate-1-2`, seen from canon: the `atip` crate's table of reserved codes and its field rules, as canon registers and declares them.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

fn canon(dir: &Path, args: &[&str]) -> Value {
    let output = hermetic::canon().args(args).current_dir(dir).env_remove("CANON_ROLE").output().expect("spawn canon");
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
    assert!(!reserved.is_empty(), "the core codes at {minor:?}");
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
    assert!(registered > 0);
    // The table is the crate's, in one place: its last row is reserved in 1.2 alone, at exit 1 and not retryable.
    assert!(!atip::RESERVED_CODES.is_empty());
    let stopped = atip::RESERVED_CODES.iter().find(|code| code.name == "STOPPED_MIDWAY").expect("a row");
    assert_eq!((stopped.exit, stopped.retryable, stopped.since, stopped.until), (1, false, atip::Minor::V1_2, Some(atip::Minor::V1_2)));
    assert!(atip::reserved_codes(atip::Minor::V1_2).any(|code| code.name == "STOPPED_MIDWAY") && !reserved.iter().any(|code| code.name == "STOPPED_MIDWAY"));
    assert!(!rows.contains_key("STOPPED_MIDWAY"), "canon registers no code of that name");
}

/// atip.crate12.18: the rules a `fields` declaration is held to are those of the minor canon declares, and a minor the crate does not know is refused.
#[test]
fn atip_crate12_18_field_rules_follow_the_declared_minor_and_a_minor_the_crate_does_not_know_is_refused() {
    use atip::fields::{check_declaration, Rules};
    let declared = Rules::for_minor(canon_cli::json_out::DECLARED_MINOR).expect("canon declares a minor the crate knows");
    assert_eq!(declared, Rules::at(atip::Minor::V1_6));
    assert_eq!(declared, Rules::for_minor("1.1").expect("a minor"), "1.1 to 1.6 keep the same rules");
    assert_ne!(declared, Rules::for_minor("1.0").expect("a minor"), "1.0 has no bracketed key, no map key and no map cut node");
    for text in ["1.9", "1.7", "2.0", "", "1"] {
        assert_eq!(Rules::for_minor(text), Err(atip::UnknownMinor(text.to_string())), "`{text}` is refused, not read as the nearest minor");
    }
    // What the rules change: a key that is not an identifier is written in brackets, which canon's declared rules allow and 1.0's refuse.
    let doc = json!({ "schemas": { "Report": { "type": "object", "properties": { "rows": { "type": "array", "items": { "$ref": "#/schemas/Row" } } } }, "Row": { "type": "object", "properties": { "attributes": { "type": "object", "additionalProperties": { "type": "string" } } } } } });
    let bracketed = |rules: Rules| check_declaration(&doc, &json!({ "$ref": "#/schemas/Report" }), "/rows", &["attributes".to_string(), "attributes[\"x.y\"]".to_string()], &[], &[], rules);
    assert_eq!(bracketed(declared), Vec::<String>::new());
    assert!(!bracketed(Rules::for_minor("1.0").expect("a minor")).is_empty(), "1.0 refuses a bracketed key");
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
    assert!(declarations > 0, "the commands that project were found");
    // Canon builds its rules by the declared minor and never by the crate's field for the old behaviour.
    let sources = sources_without_comments(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    let by_field: Vec<&PathBuf> = sources.iter().filter(|(_, code)| code.contains("v11")).map(|(path, _)| path).collect();
    assert!(by_field.is_empty(), "no source builds Rules by its old field: {by_field:?}");
}
