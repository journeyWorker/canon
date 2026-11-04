//! The names 0.33 deleted appear in no guide, website page, dashboard source, eval
//! script or design document (`atip.l0.27`). The CHANGELOG's replacement table
//! and the history of a change (`.canon/changes/`) are the only places that
//! spell them.

use std::path::{Path, PathBuf};

mod old_codes;
use old_codes::{gone_codes, gone_codes_in, registered_names, unregistered_codes_in, word_in};

/// Error codes 0.33 deleted (each has a replacement in the CHANGELOG's table).
const OLD_CODES: &[&str] = &["PLAN_CHANGED", "INPUT_ELEMENT_FAILED", "INPUT_BATCH_UNSUPPORTED", "INPUT_INVALID_TYPE", "INPUT_UNKNOWN_FIELD", "INPUT_UNREADABLE", "INPUT_STDIN_TTY", "INPUT_STDIN_CONFLICT", "INPUT_MISSING", "INVALID_CURSOR", "UNKNOWN_FIELD"];

/// Error codes 0.35 moved to the reserved midway code (`atip.midway`; each has a replacement in the CHANGELOG's table).
const MOVED_CODES: &[&str] = &["INIT_SKILL_INSTALL_FAILED"];


/// `meta` keys 0.33 deleted, each a whole word or dotted path.
const OLD_KEYS: &[&str] = &["canonVersion", "hasMore", "meta.unchanged", "meta.waitingOn", "meta.nextOmitted", "meta.hotStore", "meta.context.role", "meta.context.actor", "meta.next[].why"];

/// Old SHAPES a guide or page can show without spelling an old key: a snippet that prints what canon no
/// longer prints. Each is a line-level check, `(what, test on one line)`.
/// A test on one line of a document.
type LineTest = fn(&str) -> bool;

const OLD_SHAPES: &[(&str, LineTest)] = &[
    ("a null where canon leaves the member out", |line| ["role", "hint", "field", "next", "warnings", "context", "suggestions", "expected"].iter().any(|name| line.contains(&format!("\"{name}\": null")))),
    ("a schemaVersion that is a number, not the string \"1.0\"", |line| line.contains("\"schemaVersion\": 1,") || line.contains("\"schemaVersion\":1,")),
    ("a next entry with a flat actor/role (they are `x-canon`)", |line| line.contains("\"argv\"") && (line.contains("\"actor\":") || line.contains("\"role\":")) && !line.contains("x-canon")),
    ("a next entry with `why` (it is `reason`)", |line| line.contains("\"argv\"") && line.contains("\"why\":")),
];

/// Every old shape a line of `text` shows.
fn old_shapes_in(text: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    for line in text.lines() {
        for (what, test) in OLD_SHAPES {
            if test(line) && !found.contains(what) {
                found.push(what);
            }
        }
    }
    found
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every old name `text` spells. One spelling is not old: the quoted JSON key `"canonVersion"`
/// is a member of the `status` report's own `data` (the report is not part of this change; only
/// `meta.canonVersion` is gone), which a guide shows when it prints that report.
fn old_names_in(text: &str) -> Vec<&'static str> {
    let text = text.replace("\"canonVersion\"", "\"statusReportMember\"");
    OLD_CODES.iter().chain(gone_codes()).chain(OLD_KEYS).copied().filter(|old| word_in(&text, old)).collect()
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
    // The scenarios of this change (`area=atip`) name the old keys to say they are gone.
    let spec_skip = [skip.as_slice(), &["area=atip"]].concat();
    collect(&root.join(".canon/specs"), &["feature"], &spec_skip, &mut files);
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
        for shape in old_shapes_in(&text) {
            hits.push(format!("{} shows {shape}", file.strip_prefix(repo_root()).unwrap_or(file).display()));
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
    assert_eq!(old_names_in("CONTEXT_INPUT_NOT_FOUND is a different code"), Vec::<&str>::new());
}

#[test]
fn atip_l0_27_the_check_catches_an_old_shape_that_comes_back() {
    assert_eq!(old_shapes_in(r#"{ "reason": "r", "x-canon": { "actor": "human" } }"#), Vec::<&str>::new());
    assert_eq!(old_shapes_in(r#"{ "argv": ["a"], "x-canon": { "actor": "human", "role": null } }"#).len(), 1, "a null role");
    assert_eq!(old_shapes_in(r#""schemaVersion": 1,"#).len(), 1, "a numeric schemaVersion");
    assert_eq!(old_shapes_in(r#""schemaVersion": "1.0","#), Vec::<&str>::new());
    assert_eq!(old_shapes_in(r#"{ "argv": ["a"], "reason": "r", "actor": "agent", "role": "planner" }"#).len(), 1, "a flat actor and role");
    assert_eq!(old_shapes_in(r#"{ "argv": ["a"], "why": "r" }"#).len(), 1, "why");
}

/// Every moved code `text` spells.
fn moved_names_in(text: &str) -> Vec<&'static str> {
    MOVED_CODES.iter().chain(gone_codes()).copied().filter(|moved| word_in(text, moved)).collect()
}

/// atip.midway.11: no guide, website page, dashboard source, eval script, design document, scenario outside `area=atip`, README or
/// the sources of canon spells a code that was moved to the reserved midway code in 0.35 or replaced by a `<SUBJECT>_INCOMPLETE` code in 0.36;
/// the CHANGELOG's replacement table and the history of a change are the only places that spell them.
#[test]
fn atip_midway_11_no_consumer_spells_a_code_that_moved_or_was_replaced_by_an_incomplete_code() {
    let mut files = consumers();
    let root = repo_root();
    let skip = ["node_modules", "dist", "target", ".git", "__pycache__"];
    collect(&root.join("crates/canon-cli/src"), &["rs"], &skip, &mut files);
    collect(&root.join("conformance"), &["out", "json", "sh", "md", "txt"], &skip, &mut files);
    collect(&root.join("docs/cli"), &["json", "md"], &skip, &mut files);
    files.push(root.join("crates/canon-cli/tests/atip/canon.fixture.json"));
    assert!(files.len() > 100, "the scan found {} files: a consumer directory moved", files.len());
    let hits: Vec<String> = files
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok().map(|text| (file, text)))
        .flat_map(|(file, text)| moved_names_in(&text).into_iter().map(move |moved| format!("{} spells `{moved}`", file.strip_prefix(repo_root()).unwrap_or(file).display())))
        .collect();
    assert!(hits.is_empty(), "moved codes left in consumers: {hits:#?}");
}

/// atip.midway.11: the check catches a moved code that comes back, and does not take a longer name for it.
#[test]
fn atip_midway_11_the_check_catches_a_moved_code_that_comes_back() {
    assert_eq!(moved_names_in("exits 1 with INIT_INCOMPLETE, reason SKILLS_SOURCE_NOT_FOUND"), Vec::<&str>::new());
    assert_eq!(moved_names_in("exits 1 with STOPPED_MIDWAY, reason SKILLS_SOURCE_NOT_FOUND"), ["STOPPED_MIDWAY"], "the reserved code came back");
    assert_eq!(moved_names_in("MIGRATE_STOPPED and MIGRATE_PINS_FAILED"), ["MIGRATE_PINS_FAILED", "MIGRATE_STOPPED"]);
    assert_eq!(moved_names_in("MY_STOPPED_MIDWAY_TOO is another name, MIGRATION_INCOMPLETE is the new one"), Vec::<&str>::new());
    assert_eq!(moved_names_in("run again: INIT_SKILL_INSTALL_FAILED"), ["INIT_SKILL_INSTALL_FAILED"]);
    assert_eq!(moved_names_in(r#"{"code": "INIT_SKILL_INSTALL_FAILED", "exit": 1}"#), ["INIT_SKILL_INSTALL_FAILED"]);
    assert_eq!(moved_names_in("MY_INIT_SKILL_INSTALL_FAILED_TOO is another name"), Vec::<&str>::new());
}

/// `canon-0-36-codes-rename`, `atip.codenames.06`: no guide, website page, dashboard source, eval script, design document, scenario outside
/// `area=atip`, README or ARCHITECTURE spells an error code 0.36 renamed; the CHANGELOG's replacement table and the history of a change are
/// the only places that spell them.
#[test]
fn atip_codenames_06_no_consumer_spells_a_renamed_code() {
    let files = consumers();
    assert!(files.len() > 40, "the scan found {} files: a consumer directory moved", files.len());
    let mut hits = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        for old in gone_codes_in(&text) {
            hits.push(format!("{} spells `{old}`", file.strip_prefix(repo_root()).unwrap_or(file).display()));
        }
    }
    assert!(hits.is_empty(), "renamed codes left in consumers: {hits:#?}");
}

/// `atip.codenames.07`: a consumer that says a code canon does not have fails, a revived old name fails, and a registered code passes.
#[test]
fn atip_codenames_07_a_consumer_that_says_an_unregistered_code_or_a_revived_old_name_fails() {
    let names = registered_names();
    assert_eq!(unregistered_codes_in("refused with `ACTOR_DENIED` and `BRIEF_CHECK_FAILED`", &names), Vec::<String>::new());
    assert_eq!(unregistered_codes_in("refused with `ACTOR_DENID`", &names), ["ACTOR_DENID"], "a typo");
    assert_eq!(unregistered_codes_in("refused with `ACTOR_FORBIDDEN_INVALID`", &names), ["ACTOR_FORBIDDEN_INVALID"], "a code canon never had");
    assert_eq!(unregistered_codes_in("refused with `ACTOR_DENID_INVALID`", &names), ["ACTOR_DENID_INVALID"]);
    assert_eq!(old_names_in("a guide that still says HUMAN_ONLY"), ["HUMAN_ONLY"], "a revived old name");
    assert!(old_names_in("a guide that says ACTOR_DENIED").is_empty());
    // And the real consumers say no code canon lacks.
    let mut hits = Vec::new();
    for file in consumers() {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        for code in unregistered_codes_in(&text, &names) {
            hits.push(format!("{} says `{code}`, which canon does not register", file.strip_prefix(repo_root()).unwrap_or(&file).display()));
        }
    }
    assert!(hits.is_empty(), "consumers say codes canon does not have: {hits:#?}");
}
