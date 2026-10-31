//! T5 (0.28 D3, D4): the error registry, the spec and the sources agree.
//!
//! - the registry is the spec's `errors` list, row for row;
//! - every code a source file builds is registered (the other half, every
//!   registered code is emitted by some command, is `tests/schema_codes.rs`,
//!   and the `CANON_*` variables the crates read are accounted for in
//!   `tests/cli_spec.rs`);
//! - `canon schema --error <CODE>` explains every code, and `--errors` lists them all;
//! - every `*_NOT_FOUND`, `*_UNREADABLE` and `*_INVALID_ARGUMENT` exits 2 (D4), but `POLICY_APPROVAL_UNREADABLE`
//!   (0.33: an unreadable approval is a closed gate, exit 1);
//! - every code and exit a leaf lists is a registered code and a row of the exit table.

mod harness;

use std::collections::BTreeSet;
use std::path::Path;

use canon_cli::cli_error::all_codes;
use harness::*;

#[test]
fn the_specs_error_list_is_the_registry() {
    let listed = spec()["errors"].as_array().expect("errors");
    assert_eq!(listed.len(), all_codes().len());
    for (row, info) in listed.iter().zip(all_codes()) {
        assert_eq!(row["code"], info.code);
        assert_eq!(row["exit"], info.exit);
        assert_eq!(row["retryable"], info.retryable);
        assert_eq!(row["meaning"], info.meaning);
        assert_eq!(row["fix"], info.fix);
    }
    let mut sorted: Vec<&str> = all_codes().iter().map(|i| i.code).collect();
    let before = sorted.len();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(before, sorted.len(), "a code is registered twice");
}

fn live_sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            live_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).expect("read source");
            let live = text.split("#[cfg(test)]").next().expect("a piece").to_string();
            out.push((path.display().to_string(), live));
        }
    }
}

/// Every string literal a source hands to `CliError::refused(` / `::failed(` (the constructors of a coded error).
#[test]
fn every_code_a_source_builds_is_registered() {
    let registry: BTreeSet<&str> = all_codes().iter().map(|i| i.code).collect();
    let mut sources = Vec::new();
    live_sources(&repo_root().join("crates/canon-cli/src"), &mut sources);
    let (mut seen, mut problems) = (0, Vec::new());
    for (file, text) in &sources {
        for needle in ["CliError::refused(\"", "CliError::failed(\"", "refused(\"", "failed(\""] {
            for (at, _) in text.match_indices(needle) {
                let rest = &text[at + needle.len()..];
                let code: String = rest.chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').collect();
                if code.is_empty() || !rest[code.len()..].starts_with('"') {
                    continue;
                }
                seen += 1;
                if !registry.contains(code.as_str()) {
                    problems.push(format!("{file}: `{code}` is built but not registered in cli_error::all_codes()"));
                }
            }
        }
    }
    assert!(seen > 100, "the scan saw the constructors ({seen})");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn not_found_unreadable_and_invalid_argument_exit_2() {
    let mut checked = 0;
    for info in all_codes() {
        if ["_NOT_FOUND", "_UNREADABLE", "_INVALID_ARGUMENT"].iter().any(|suffix| info.code.ends_with(suffix)) && info.code != "POLICY_APPROVAL_UNREADABLE" {
            checked += 1;
            assert_eq!(info.exit, 2, "{} must exit 2 (D4)", info.code);
        }
    }
    assert!(checked > 20, "the registry has the families ({checked})");
}

#[test]
fn a_leaf_lists_only_registered_codes_and_the_exits_of_the_table() {
    let registry: BTreeSet<&str> = all_codes().iter().map(|i| i.code).collect();
    let table: BTreeSet<u64> = spec()["exitTable"].as_array().unwrap().iter().map(|r| r["code"].as_u64().unwrap()).collect();
    for node in leaves() {
        let path = path_of(node);
        for code in strings(&node["errorCodes"]) {
            assert!(registry.contains(code.as_str()), "`{path}` lists {code}, which is not registered");
        }
        let exits: Vec<u64> = node["exits"].as_array().unwrap().iter().map(|e| e["code"].as_u64().unwrap()).collect();
        assert!(!exits.is_empty() && exits.iter().all(|e| table.contains(e)), "`{path}` exits {exits:?}");
        assert!(exits.contains(&0), "`{path}` has no success exit");
        // A leaf that lists a code of exit 2 declares exit 2; one that lists a code of exit 1 declares exit 1.
        for code in strings(&node["errorCodes"]) {
            let exit = u64::from(all_codes().iter().find(|i| i.code == code).unwrap().exit);
            assert!(exits.contains(&exit), "`{path}` lists {code} (exit {exit}) but declares only {exits:?}");
        }
    }
}

#[test]
fn schema_explains_every_code_and_lists_them_all() {
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let listing = document(&canon_in(dir.path(), home.path(), &strs(&["schema", "--errors", "--json"]), &[]));
    let listed: Vec<String> = listing["data"]["errors"].as_array().expect("--errors lists the codes").iter().map(|e| e["code"].as_str().unwrap().to_string()).collect();
    assert_eq!(listed, all_codes().iter().map(|i| i.code.to_string()).collect::<Vec<_>>());
    for info in all_codes() {
        let out = canon_in(dir.path(), home.path(), &strs(&["schema", "--error", info.code, "--json"]), &[]);
        assert!(out.status.success(), "schema --error {}: {}", info.code, String::from_utf8_lossy(&out.stdout));
        let body = document(&out);
        assert_eq!(body["data"]["error"]["code"], info.code);
        assert_eq!(body["data"]["error"]["exit"], info.exit);
        assert_eq!(body["data"]["error"]["retryable"], info.retryable);
    }
    let unknown = canon_in(dir.path(), home.path(), &strs(&["schema", "--error", "NO_SUCH_CODE", "--json"]), &[]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(document(&unknown)["error"]["code"], "UNKNOWN_ERROR_CODE");
}
