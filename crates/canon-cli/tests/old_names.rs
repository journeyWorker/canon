//! The names 0.33 deleted appear in no guide, website page, dashboard source, eval
//! script or design document (`atip.l0.27`). The CHANGELOG's replacement table
//! and the history of a change (`.canon/changes/`) are the only places that
//! spell them.

use std::path::{Path, PathBuf};

/// Error codes 0.33 deleted (each has a replacement in the CHANGELOG's table).
const OLD_CODES: &[&str] = &["PLAN_CHANGED", "INPUT_ELEMENT_FAILED", "INPUT_BATCH_UNSUPPORTED", "INPUT_INVALID_TYPE", "INPUT_UNKNOWN_FIELD", "INPUT_UNREADABLE", "INPUT_STDIN_TTY", "INPUT_STDIN_CONFLICT", "INPUT_MISSING", "INVALID_CURSOR", "UNKNOWN_FIELD"];

/// `meta` keys 0.33 deleted, each a whole word or dotted path.
const OLD_KEYS: &[&str] = &["canonVersion", "hasMore", "meta.unchanged", "meta.waitingOn", "meta.nextOmitted", "meta.hotStore", "meta.context.role", "meta.context.actor", "meta.next[].why"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `word` in `text` as a whole identifier, so `CONTEXT_INPUT_MISSING` is not `INPUT_MISSING`.
fn word_in(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        !part(before) && !part(after)
    })
}

/// Every old name `text` spells. One spelling is not old: the quoted JSON key `"canonVersion"`
/// is a member of the `status` report's own `data` (the report is not part of this change; only
/// `meta.canonVersion` is gone), which a guide shows when it prints that report.
fn old_names_in(text: &str) -> Vec<&'static str> {
    let text = text.replace("\"canonVersion\"", "\"statusReportMember\"");
    OLD_CODES.iter().chain(OLD_KEYS).copied().filter(|old| word_in(&text, old)).collect()
}

fn collect(dir: &Path, extensions: &[&str], skip: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if skip.contains(&name.as_str()) {
            continue;
        }
        if path.is_dir() {
            collect(&path, extensions, skip, out);
        } else if path.extension().is_some_and(|e| extensions.contains(&e.to_string_lossy().as_ref())) {
            out.push(path);
        }
    }
}

/// Where a reader of canon JSON or text looks: the guides the binary carries, the website, the
/// dashboard, the eval scripts, the launcher and the design documents.
fn consumers() -> Vec<PathBuf> {
    let root = repo_root();
    let skip = ["node_modules", "dist", "target", ".git", "__pycache__"];
    let mut files = Vec::new();
    collect(&root.join("canon/skills"), &["md", "sh", "json"], &skip, &mut files);
    collect(&root.join("packages/website/src"), &["md", "mdx", "ts", "tsx", "astro", "json"], &skip, &mut files);
    collect(&root.join("packages/dashboard/src"), &["ts", "tsx", "js", "css", "html"], &skip, &mut files);
    collect(&root.join("packages/cli/src"), &["ts", "js"], &skip, &mut files);
    collect(&root.join("evals"), &["py", "jsonl", "md", "json", "sh"], &skip, &mut files);
    collect(&root.join("docs/design"), &["md"], &skip, &mut files);
    files.extend(["README.md", "ARCHITECTURE.md"].iter().map(|name| root.join(name)));
    files
}

#[test]
fn atip_l0_27_no_guide_page_dashboard_or_eval_script_spells_an_old_name() {
    let files = consumers();
    assert!(files.len() > 40, "the scan found {} files: a consumer directory moved", files.len());
    let mut hits = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        for old in old_names_in(&text) {
            hits.push(format!("{} spells `{old}`", file.strip_prefix(repo_root()).unwrap_or(file).display()));
        }
    }
    assert!(hits.is_empty(), "old names left in consumers: {hits:#?}");
}

#[test]
fn atip_l0_27_the_check_catches_an_old_name_that_comes_back() {
    assert_eq!(old_names_in("the page lists PLAN_STALE and CURSOR_INVALID"), Vec::<&str>::new());
    assert_eq!(old_names_in("run again: PLAN_CHANGED"), ["PLAN_CHANGED"]);
    assert_eq!(old_names_in("read `canonVersion` and hasMore"), ["canonVersion", "hasMore"]);
    assert_eq!(old_names_in("meta.canonVersion is gone"), ["canonVersion"], "the meta key is old whatever path it is written in");
    assert_eq!(old_names_in(r#"{ "canonVersion": "0.29.0" } is the status report's member"#), Vec::<&str>::new());
    assert_eq!(old_names_in("CONTEXT_INPUT_MISSING is a different code"), Vec::<&str>::new());
}
