//! atip/1 1.1 renamed things canon 0.33 printed: the reason of an `--input` fault moved from the extension
//! `x-canon.reason` (lowercase values) to the contract's `error.reason` (reserved UPPER_SNAKE names), and
//! `UNKNOWN_ERROR_CODE` merged into `USAGE`. The old spellings are gone from everything a caller or an agent
//! reads: the guides the binary ships, the website in both languages, the generated command spec, the
//! conformance corpus, the evaluation scripts and the scenario sentences. This test fails when one is revived;
//! the CHANGELOG names them on purpose (its replacement table) and is not scanned.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The first old name `text` holds, if any.
fn old_name(text: &str) -> Option<&'static str> {
    if text.contains("x-canon.reason") {
        return Some("x-canon.reason");
    }
    if text.contains("UNKNOWN_ERROR_CODE") {
        return Some("UNKNOWN_ERROR_CODE");
    }
    // `"x-canon":{"reason":…}` on an error object, in a frozen document or an example.
    let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if squeezed.contains("\"x-canon\":{\"reason\"") {
        return Some("an x-canon extension that carries a reason");
    }
    // The lowercase values of the 0.33 reason, as the value of a `reason` member.
    for old in ["unreadable", "tty", "empty", "syntax", "batch-unsupported"] {
        if squeezed.contains(&format!("\"reason\":\"{old}\"")) {
            return Some("a lowercase 0.33 reason value");
        }
    }
    None
}

fn files_under(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.map(|e| e.expect("dir entry").path()).collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if path.is_dir() {
            if !matches!(name, "node_modules" | "__pycache__" | "target" | ".git" | "dist") {
                files_under(&path, extensions, out);
            }
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| extensions.contains(&e)) {
            out.push(path);
        }
    }
}

/// Every file the old names must be gone from, by surface.
fn surfaces() -> Vec<(&'static str, Vec<PathBuf>)> {
    let root = repo_root();
    let mut out = Vec::new();
    let mut collect = |label: &'static str, dirs: &[&str], extensions: &[&str]| {
        let mut files = Vec::new();
        for dir in dirs {
            let path = root.join(dir);
            if path.is_file() {
                files.push(path);
            } else {
                files_under(&path, extensions, &mut files);
            }
        }
        out.push((label, files));
    };
    collect("the guides the binary ships", &["canon/skills", "README.md", "ARCHITECTURE.md"], &["md"]);
    collect("the website, English and Korean", &["packages/website/src"], &["md", "mdx"]);
    collect("the command spec files", &["docs/cli", "crates/canon-cli/tests/atip"], &["json"]);
    collect("the conformance corpus", &["conformance"], &["out", "err", "yaml", "md", "sh"]);
    collect("the evaluation scripts", &["evals"], &["py", "json", "md", "tsv"]);
    collect("the scenario sentences", &[".canon/specs"], &["feature"]);
    out
}

#[test]
fn atip_contract11_13_the_0_33_names_are_gone_from_every_guide_page_script_spec_and_scenario() {
    let mut scanned = 0;
    let mut revived = Vec::new();
    for (label, files) in surfaces() {
        assert!(!files.is_empty(), "{label}: the scan found no file; it cannot be looking at the repository");
        for file in files {
            let text = fs::read_to_string(&file).unwrap_or_default();
            scanned += 1;
            if let Some(name) = old_name(&text) {
                revived.push(format!("{label}: {} holds {name}", file.strip_prefix(repo_root()).unwrap_or(&file).display()));
            }
        }
    }
    assert!(scanned > 200, "the scan read {scanned} files");
    assert!(revived.is_empty(), "0.33 names that came back:\n{}", revived.join("\n"));
}

#[test]
fn atip_contract11_13_reviving_one_of_the_old_names_fails_the_check() {
    assert_eq!(old_name("`error.x-canon.reason` is `tty`"), Some("x-canon.reason"));
    assert_eq!(old_name("canon schema --error NOPE exits 2 with UNKNOWN_ERROR_CODE"), Some("UNKNOWN_ERROR_CODE"));
    assert!(old_name(r#"{"error":{"code":"INPUT_INVALID","x-canon":{ "reason": "tty" }}}"#).is_some());
    assert!(old_name(r#"{"error":{"code":"INPUT_INVALID","reason":"batch-unsupported"}}"#).is_some());
    assert!(old_name(r#"{"error":{"code":"INPUT_INVALID","reason":"STDIN_TTY"}}"#).is_none(), "the new names pass");
    assert!(old_name("a reason given with --reason is free text, and the file is empty").is_none(), "prose that uses the words is not a revived name");
}
