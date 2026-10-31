//! atip.l0.13: every warning carries a registered code. A warning is an entry
//! of `meta.warnings` (`{code, message, field?}`, spec 8.4); the code is
//! registered in `cli_error::all_codes()` like an error code. One scan keeps
//! the sources honest (no warning site names an unregistered code), and one
//! status run shows the codes reaching the envelope.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Every `.rs` file under `dir`, recursively, as `(path, text)`.
fn sources_under(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources_under(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            out.push((path, text));
        }
    }
}

fn screaming(s: &str) -> bool {
    !s.is_empty() && s.starts_with(|c: char| c.is_ascii_uppercase()) && s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// `(file, code)` for every warning site whose first argument is a code
/// literal: `warn_err!("CODE"`, `json_out::warn("CODE"`, `warn_at("CODE"`,
/// `Warning::new("CODE"`, a `warnings.push(("CODE"` of the search index, and
/// the `warning("CODE"` helper of the policy approval advisory.
fn warning_sites() -> Vec<(String, String)> {
    let mut files = Vec::new();
    sources_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut found = Vec::new();
    for (path, text) in files {
        let name = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap().display().to_string();
        for marker in ["warn_err!(", "json_out::warn(", "warn_at(", "Warning::new(", "warnings.push((", "warning("] {
            for (at, _) in text.match_indices(marker) {
                let rest = text[at + marker.len()..].trim_start();
                let Some(quoted) = rest.strip_prefix('"') else { continue };
                let Some((literal, _)) = quoted.split_once('"') else { continue };
                if screaming(literal) {
                    found.push((name.clone(), literal.to_string()));
                }
            }
        }
    }
    found
}

#[test]
fn atip_l0_13_every_warning_site_names_a_registered_code() {
    let registered: BTreeSet<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    let sites = warning_sites();
    assert!(sites.len() >= 20, "the scan found only {} warning sites; it cannot be looking at the sources", sites.len());
    let unregistered: Vec<_> = sites.iter().filter(|(_, code)| !registered.contains(code.as_str())).collect();
    assert!(unregistered.is_empty(), "warning codes used but not in cli_error::all_codes(): {unregistered:?}");
}

#[test]
fn atip_l0_13_no_warning_macro_call_lacks_a_code_literal() {
    let mut files = Vec::new();
    sources_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut bare = Vec::new();
    for (path, text) in files {
        for (at, _) in text.match_indices("warn_err!(") {
            let rest = text[at + "warn_err!(".len()..].trim_start();
            if !rest.starts_with('"') {
                bare.push(format!("{}: {}", path.display(), rest.lines().next().unwrap_or_default()));
            }
        }
    }
    // The macro's own definition (`macro_rules! warn_err {`) has no `(` after the name.
    assert!(bare.is_empty(), "`warn_err!` calls without a code literal as the first argument: {bare:?}");
}

#[test]
fn atip_l0_13_a_warning_code_has_the_registry_shape_of_an_error_code() {
    for (_, code) in warning_sites() {
        let info = canon_cli::cli_error::code_info(&code).unwrap_or_else(|| panic!("{code} is not registered"));
        assert!(matches!(info.exit, 1 | 2), "{code} declares exit {}", info.exit);
        assert!(!info.retryable, "{code} is a warning: a retry changes nothing");
        assert!(info.fix.ends_with('.') && !info.fix.trim().is_empty(), "{code}: `fix` is one sentence: {}", info.fix);
    }
}

#[test]
fn atip_l0_13_every_warning_the_status_report_produces_has_a_registered_code() {
    // An empty directory is the cheap state that makes status warn: no
    // canon.yaml, no policy, an unreadable ledger.
    let dir = tempfile::TempDir::new().unwrap();
    let report = canon_cli::status::resolve(dir.path());
    assert!(!report.coded_warnings.is_empty(), "an empty directory must make `status` warn");
    for warning in &report.coded_warnings {
        assert!(canon_cli::cli_error::code_info(&warning.code).is_some(), "status warning code {} is not registered: {}", warning.code, warning.message);
    }
    // `data.warnings` keeps its shape (plain lines); the coded list carries the same text, in the same order.
    let lines: Vec<&str> = report.coded_warnings.iter().map(|w| w.message.as_str()).collect();
    assert_eq!(report.warnings.iter().map(String::as_str).collect::<Vec<_>>(), lines);
}

#[test]
fn atip_l0_13_status_json_carries_coded_warnings_in_meta_and_plain_lines_in_data() {
    let dir = tempfile::TempDir::new().unwrap();
    let init = Command::new(env!("CARGO_BIN_EXE_canon")).args(["init", "--providers", "none", "--no-policy"]).current_dir(dir.path()).output().expect("spawn canon binary");
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["status", "--json"]).current_dir(dir.path()).output().expect("spawn canon binary");
    let document: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout)));
    let warnings = document["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("no meta.warnings: {document}"));
    assert!(!warnings.is_empty(), "{document}");
    for warning in warnings {
        let code = warning["code"].as_str().unwrap_or_else(|| panic!("a warning without a code: {warning}"));
        assert!(canon_cli::cli_error::code_info(code).is_some(), "meta.warnings code {code} is not registered: {warning}");
        assert!(warning["message"].as_str().is_some_and(|m| !m.is_empty()), "{warning}");
    }
    if let Some(lines) = document.get("data").and_then(|data| data.get("warnings")).and_then(Value::as_array) {
        for line in lines {
            let line = line.as_str().expect("data.warnings holds plain strings");
            assert!(warnings.iter().any(|w| w["message"] == line), "data.warnings line has no coded twin in meta.warnings: {line}");
        }
    }
}
