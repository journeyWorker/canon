//! T9 (0.28 D2): every output the spec declares was emitted at least once by the suite.
//!
//! A debug build of `canon` appends one line per JSON document it prints to
//! `$CANON_WITNESS_DIR/<pid>.ndjson` (`json_out::witness`): the command, `ok`, and the
//! report's Rust type. `scripts/check-witness.sh` runs the whole workspace suite with the
//! variable set and then this file's ignored test, which holds those lines against the
//! spec: for every leaf, each declared report was seen with `ok` true or false, and a
//! failure envelope was seen. What the suite does not reach is named in
//! [`UNWITNESSED`], with the reason; the list only shrinks: an entry the suite now
//! reaches fails the check until it is removed.
//!
//! Always on: the recorder works, and the allowlist names real outputs with reasons.
//! `#[ignore]`d (needs the witness of a whole suite run): the coverage check.

mod harness;

use std::collections::BTreeSet;
use std::path::PathBuf;

use harness::*;
use serde_json::Value;
use tempfile::TempDir;

/// The output of a leaf the suite never emits, as (leaf, report name or `failure`, why).
const UNWITNESSED: &[(&str, &str, &str)] = &[];

/// What one leaf can emit: each declared report, and its failure envelope.
fn declared() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for node in leaves() {
        let path = path_of(node);
        for output in node["outputs"].as_array().unwrap() {
            out.push((path.clone(), output["report"].as_str().unwrap().to_string()));
        }
        out.push((path, "failure".to_string()));
    }
    out
}

/// (command, ok, Rust type) of every recorded document under `dir`.
fn recorded(dir: &PathBuf) -> Vec<(String, bool, String)> {
    let mut lines = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ndjson") {
            for line in std::fs::read_to_string(&path).unwrap().lines() {
                let v: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{}: {e}: {line}", path.display()));
                lines.push((v["command"].as_str().unwrap_or_default().to_string(), v["ok"] == true, v["type"].as_str().unwrap_or_default().to_string()));
            }
        }
    }
    lines
}

/// The declared outputs `lines` did not witness.
fn missing(lines: &[(String, bool, String)]) -> Vec<(String, String)> {
    let rows = canon_cli::reports::report_schemas();
    declared()
        .into_iter()
        .filter(|(leaf, output)| {
            if output == "failure" {
                return !lines.iter().any(|(command, ok, _)| command == leaf && !ok);
            }
            let row = rows.iter().find(|r| r.command == leaf && &r.name == output).unwrap_or_else(|| panic!("`{leaf}` declares {output}, which has no report_schemas() row"));
            !lines.iter().any(|(command, _, kind)| command == leaf && kind == row.type_name)
        })
        .collect()
}

#[test]
fn a_debug_build_records_each_document_it_prints() {
    if !cfg!(debug_assertions) {
        return; // a release binary records nothing
    }
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let witness = TempDir::new().unwrap();
    let dir = witness.path();
    let envs = [("CANON_WITNESS_DIR", dir.to_str().unwrap())];
    let success = canon_in(base.path(), home.path(), &strs(&["status", "--json"]), &envs);
    assert!(success.status.success());
    let failure = canon_in(base.path(), home.path(), &strs(&["brief", "show", "no-such-change", "--json"]), &envs);
    assert_eq!(failure.status.code(), Some(2));
    // Without the variable nothing is recorded.
    canon_in(base.path(), home.path(), &strs(&["status", "--json"]), &[]);

    let lines = recorded(&dir.to_path_buf());
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines.contains(&("status".to_string(), true, "canon_cli::status::StatusReport".to_string())), "{lines:?}");
    assert!(lines.iter().any(|(command, ok, _)| command == "brief show" && !ok), "{lines:?}");
    // The check reads those lines: `status` is witnessed, its failure is not.
    let missing = missing(&lines);
    assert!(!missing.contains(&("status".to_string(), "StatusReport".to_string())));
    assert!(missing.contains(&("status".to_string(), "failure".to_string())));
    assert!(!missing.contains(&("brief show".to_string(), "failure".to_string())));
}

#[test]
fn the_allowlist_names_real_outputs_and_says_why() {
    let declared: BTreeSet<(String, String)> = declared().into_iter().collect();
    let mut seen = BTreeSet::new();
    for (leaf, output, reason) in UNWITNESSED {
        assert!(!reason.trim().is_empty(), "`{leaf}` {output}: an allowlist entry needs its reason");
        assert!(declared.contains(&(leaf.to_string(), output.to_string())), "UNWITNESSED names `{leaf}` {output}, which the spec does not declare");
        assert!(seen.insert((leaf, output)), "`{leaf}` {output} is listed twice");
    }
}

/// Needs `CANON_WITNESS_DIR` filled by a whole suite run: `scripts/check-witness.sh`.
#[test]
#[ignore = "needs the witness of a whole suite run; run scripts/check-witness.sh"]
fn the_suite_emitted_every_declared_output() {
    let dir = PathBuf::from(std::env::var_os("CANON_WITNESS_DIR").expect("CANON_WITNESS_DIR names the directory the suite recorded into"));
    let lines = recorded(&dir);
    assert!(lines.len() > 500, "the suite recorded {} documents: was it run with CANON_WITNESS_DIR set, on a debug build?", lines.len());
    let missing = missing(&lines);
    let allowed: BTreeSet<(String, String)> = UNWITNESSED.iter().map(|(leaf, output, _)| (leaf.to_string(), output.to_string())).collect();
    let uncovered: Vec<String> = missing.iter().filter(|m| !allowed.contains(*m)).map(|(leaf, output)| format!("`{leaf}` {output}")).collect();
    let stale: Vec<String> = allowed.iter().filter(|a| !missing.contains(a)).map(|(leaf, output)| format!("`{leaf}` {output}")).collect();
    assert!(uncovered.is_empty(), "declared outputs no test emitted (cover them, or allowlist with a reason):\n{}", uncovered.join("\n"));
    assert!(stale.is_empty(), "now witnessed, so drop them from UNWITNESSED:\n{}", stale.join("\n"));
}
