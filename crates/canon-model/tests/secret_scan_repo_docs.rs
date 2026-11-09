//! `canon-0-39-secret-scan`, F1: the prose of this repository holds no secret shape. `canon/`, `docs/` and `.canon/` are the documents an
//! agent selects into a context pack, and 0.38 refused 26 of their 176 `.md` files for words such as `risk-high` and `task-list`.

use std::path::{Path, PathBuf};

use canon_model::secret_scan::{line_of, scan};

fn markdown_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            markdown_under(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            found.push(path);
        }
    }
}

/// dispatch.secret-scan.03 (F1): no `.md` under `canon/`, `docs/` and `.canon/` holds a secret span.
#[test]
fn dispatch_secret_scan_03_no_markdown_file_of_the_repository_holds_a_secret_span() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["canon", "docs", ".canon"] {
        markdown_under(&root.join(dir), &mut files);
    }
    assert!(files.len() > 100, "the walk found only {} markdown files: the directories moved", files.len());
    let mut hits = Vec::new();
    for file in &files {
        let text = String::from_utf8_lossy(&std::fs::read(file).expect("a readable markdown file")).into_owned();
        for span in scan(&text) {
            hits.push(format!("{} line {} ({})", file.strip_prefix(&root).unwrap_or(file).display(), line_of(&text, span.start), span.kind.rule_name()));
        }
    }
    assert!(hits.is_empty(), "markdown files the scanner would refuse:\n{}", hits.join("\n"));
}
