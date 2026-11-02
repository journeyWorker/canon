//! canon-0-35-crate-1-2, seen from canon: the `atip` crate moves to 1.2 and canon keeps declaring 1.1.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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
fn atip_crate12_02_canon_still_declares_contract_version_1_1_though_the_crate_is_at_1_2() {
    assert_eq!(atip::CONTRACT_VERSION, "1.2", "the crate implements 1.2");
    assert_eq!(canon_cli::json_out::DECLARED_MINOR, "1.1", "canon declares 1.1 until it adopts 1.2");
    let dir = tempfile::tempdir().expect("a temp dir");
    let schema = canon(dir.path(), &["schema", "--json"]);
    assert_eq!(schema["meta"]["contractVersion"], "1.1", "{schema}");
    assert_eq!(schema["meta"]["schemaVersion"], "1.1", "the self-description follows the declared minor");
    // A refusal declares it too: `status` finds no canon.yaml here.
    let status = canon(dir.path(), &["status", "--json"]);
    assert_eq!((status["ok"].clone(), status["meta"]["contractVersion"].clone()), (Value::Bool(false), Value::from("1.1")), "{status}");
    assert!(atip::envelope_problems(&status).is_empty(), "a 1.1 document stays valid against the vendored schema");
}

#[test]
fn atip_crate12_03_no_source_of_canon_reads_the_crates_contract_version() {
    let sources = sources_without_comments(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    assert!(sources.len() > 50, "the walk found canon's sources");
    let readers: Vec<&PathBuf> = sources.iter().filter(|(_, code)| code.contains("CONTRACT_VERSION")).map(|(path, _)| path).collect();
    assert!(readers.is_empty(), "canon declares its own minor (`json_out::DECLARED_MINOR`); the crate's CONTRACT_VERSION is read in {readers:?}");
}
