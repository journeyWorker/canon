//! `canon-0-36-codes-table`: every error code of the registry follows the contract's name rule (`atip.codestable.05`, `.06`). Tests are named by scenario
//! id (`atip_codestable_05_…` proves `atip.codestable.05`).

use std::path::{Path, PathBuf};

use atip::Minor;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every error code of the registry follows the contract's rule, as `(code, exit, retryable)`: the problems the rule names.
fn rule_problems<'a>(codes: impl IntoIterator<Item = (&'a str, u8, bool)>) -> Vec<String> {
    codes.into_iter().filter_map(|(code, exit, retryable)| atip::code_name_problem(Minor::V1_3, code, Some(i64::from(exit)), Some(retryable))).collect()
}

/// atip.codestable.05: every code of the registry follows the name rule and no test names a list of codes that do not follow it yet.
#[test]
fn atip_codestable_05_every_code_follows_the_name_rule_and_no_list_of_exceptions_exists() {
    let problems = rule_problems(canon_cli::cli_error::all_codes().iter().map(|info| (info.code, info.exit, info.retryable)));
    assert!(problems.is_empty(), "codes break the name rule: {problems:#?}");
    // The two lists of codes still to rename or reclassify that the first changes kept are gone, and no source names them.
    let needles = [["NOT_YET", "_RENAMED"].concat(), ["NOT_YET", "_RECLASSIFIED"].concat()];
    let mut files = Vec::new();
    collect(&repo_root().join("crates"), &["rs"], &["target", "node_modules"], &mut files);
    assert!(!files.is_empty(), "the scan found no files");
    let hits: Vec<String> = files
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok().map(|text| (file, text)))
        .flat_map(|(file, text)| needles.iter().filter(move |needle| text.contains(needle.as_str())).map(move |needle| format!("{} names {needle}", file.display())))
        .collect();
    assert!(hits.is_empty(), "a list of exceptions to the name rule is back: {hits:#?}");
}

/// atip.codestable.06: a name without a condition, a wrong exit and a wrong retryable value are each found by the rule the registry check uses.
#[test]
fn atip_codestable_06_a_code_that_breaks_the_name_rule_is_found() {
    assert_eq!(rule_problems([("ACTOR_DENIED", 2, false), ("REPO_LOCK_UNAVAILABLE", 2, true)]), Vec::<String>::new());
    assert_eq!(rule_problems([("FINDING_STAGED", 2, false)]).len(), 1, "no condition");
    assert_eq!(rule_problems([("COMMIT_REQUIRED", 1, false)]).len(), 1, "a REQUIRED code is exit 2");
    assert_eq!(rule_problems([("REPO_LOCK_UNAVAILABLE", 2, false)]).len(), 1, "an UNAVAILABLE code is retryable");
    assert_eq!(rule_problems([("HUMAN_ONLY", 2, false), ("NO_COMMIT", 2, false), ("UNKNOWN_ROLE", 2, false)]).len(), 3);
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
        } else if extensions.is_empty() || path.extension().is_some_and(|e| extensions.contains(&e.to_string_lossy().as_ref())) {
            out.push(path);
        }
    }
}
