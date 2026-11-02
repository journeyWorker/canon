//! canon-0-35-crate-1-2, seen from the checker: the `atip` crate moves to 1.2, the checker keeps declaring 1.1 and reads the
//! reserved codes from the crate.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use atip::Minor;
use atip_check::checks::documents::registry_findings;

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

fn checker_sources() -> Vec<(PathBuf, String)> {
    let found = sources_without_comments(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    assert!(found.len() > 10, "the walk found the checker's sources");
    found
}

#[test]
fn atip_crate12_02_the_checker_declares_its_own_contract_version_apart_from_the_crates() {
    assert_eq!(atip::CONTRACT_VERSION, "1.2", "the crate implements 1.2");
    assert_eq!(atip_check::output::DECLARED_MINOR, "1.2", "the checker declares 1.2 since canon-0-35-check-1-2");
    let line = atip_check::output::success_line(&json!({}), 0);
    let envelope: Value = serde_json::from_str(&line).expect("one JSON line");
    assert_eq!(envelope["meta"]["contractVersion"], "1.2", "{line}");
    assert!(atip::envelope_problems(&envelope).is_empty(), "{line}");
}

#[test]
fn atip_crate12_03_no_source_of_the_checker_reads_the_crates_contract_version() {
    let readers: Vec<PathBuf> = checker_sources().into_iter().filter(|(_, code)| code.contains("CONTRACT_VERSION")).map(|(path, _)| path).collect();
    assert!(readers.is_empty(), "the checker declares its own minor (`output::DECLARED_MINOR`); the crate's CONTRACT_VERSION is read in {readers:?}");
}

#[test]
fn atip_crate12_04_the_checker_keeps_no_table_of_the_reserved_codes() {
    for (path, code) in checker_sources() {
        assert!(!code.contains("RESERVED_CODES"), "{path:?} holds a table of the reserved codes: the crate's is the only one");
        assert!(!code.contains("(\"UNKNOWN_COMMAND\", 2)") && !code.contains("(\"BATCH_INCOMPLETE\", 1)"), "{path:?} copies the reserved rows");
    }
    // It reads them from the crate: a registry that gets a reserved exit wrong is found, for every row of the table.
    for reserved in atip::RESERVED_CODES.iter().filter(|reserved| reserved.since == Minor::V1_0 && !["USAGE", "UNKNOWN_COMMAND", "COMMAND_REQUIRED", "INTERNAL"].contains(&reserved.name)) {
        let wrong_exit = i64::from(if reserved.exit == 1 { 2 } else { 1 });
        let entries = vec![json!({ "code": reserved.name, "exit": wrong_exit, "retryable": false })];
        let findings: Vec<_> = registry_findings(&entries, Minor::V1_1).into_iter().filter(|(_, problem)| problem.is_some()).collect();
        assert_eq!(findings.len(), 1, "{}: {findings:?}", reserved.name);
        assert_eq!(findings[0].0, "reserved-exit");
    }
}

#[test]
fn atip_crate12_05_stopped_midway_with_exit_2_is_found_in_a_1_2_registry_and_not_in_a_1_1_one() {
    let entries = vec![json!({ "code": "STOPPED_MIDWAY", "exit": 2, "retryable": false })];
    let problems_at = |minor: Minor| -> Vec<(&'static str, String)> { registry_findings(&entries, minor).into_iter().filter_map(|(condition, problem)| problem.map(|problem| (condition, problem))).collect() };
    let at_12 = problems_at(Minor::V1_2);
    assert_eq!(at_12.len(), 1, "{at_12:?}");
    assert_eq!(at_12[0].0, "reserved-exit");
    assert!(at_12[0].1.contains("STOPPED_MIDWAY") && at_12[0].1.contains("reserved is exit 1"), "{at_12:?}");
    assert!(problems_at(Minor::V1_1).is_empty(), "in a 1.1 document the code is the tool's own");
    assert!(problems_at(Minor::V1_0).is_empty());
    // The code as the contract reserves it passes in a 1.2 registry.
    let reserved = vec![json!({ "code": "STOPPED_MIDWAY", "exit": 1, "retryable": false })];
    assert!(registry_findings(&reserved, Minor::V1_2).iter().all(|(_, problem)| problem.is_none()));
}
