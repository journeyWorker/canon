//! `canon-0-36-codes-table`: the CHANGELOG's replacement table, the four lists and the upgrade note that close the 0.36 renaming of the error codes, and
//! the proof that no consumer spells an old name. Tests are named by scenario id (`atip_codestable_01_…` proves `atip.codestable.01`).
//!
//! The 0.35 registry is test data (`tests/fixtures/codes-0.35.tsv`: name, exit, retryable of each of its 179 codes), because the old names are in no
//! registry view any more. The table is read from the CHANGELOG by `old_codes::replacement_table` and checked against that data and the registry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use atip::Minor;

mod old_codes;
use old_codes::{changelog, gone_codes, gone_codes_in, registered_names, registry_0_35, replacement_table, unregistered_codes_in, Row};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn registry_row(code: &str) -> Option<&'static atip::CodeInfo> {
    canon_cli::cli_error::all_codes().iter().find(|info| info.code == code)
}

fn sorted_unique<T: Ord>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut values: Vec<T> = values.into_iter().collect();
    values.sort_unstable();
    values.dedup();
    values
}

/// What is wrong with the replacement table of `changelog`: its rows against the 0.35 data and the registry. Empty when the table is the registry's.
fn table_problems(changelog: &str) -> Vec<String> {
    let rows = match replacement_table(changelog) {
        Ok(rows) => rows,
        Err(problem) => return vec![problem],
    };
    let mut problems = Vec::new();
    let fixture: BTreeMap<&str, (u8, bool)> = registry_0_35().iter().map(|(code, exit, retryable)| (*code, (*exit, *retryable))).collect();
    // Each old name is in the table exactly once.
    let mut seen = BTreeSet::new();
    for row in &rows {
        if !seen.insert(row.old.as_str()) {
            problems.push(format!("`{}` has two rows", row.old));
        }
        if !fixture.contains_key(row.old.as_str()) {
            problems.push(format!("`{}` is no code of the 0.35 registry", row.old));
        }
    }
    for code in fixture.keys().filter(|code| !seen.contains(*code)) {
        problems.push(format!("the table has no row for `{code}`"));
    }
    let targets: BTreeMap<&str, usize> = rows.iter().filter(|row| row.new.len() == 1 && row.new[0] != row.old).fold(BTreeMap::new(), |mut count, row| {
        *count.entry(row.new[0].as_str()).or_default() += 1;
        count
    });
    for row in &rows {
        problems.extend(row_problems(row, &fixture, &targets));
    }
    // Every code of the registry is the new name or the kept name of some row, or was registered after 0.36.
    let named: BTreeSet<&str> = rows.iter().flat_map(|row| row.new.iter().map(String::as_str)).collect();
    for info in canon_cli::cli_error::all_codes().iter().filter(|info| !old_codes::ADDED_AFTER_0_36.contains(&info.code)) {
        if !named.contains(info.code) {
            problems.push(format!("the registry's `{}` is the new name of no row", info.code));
        }
    }
    problems
}

fn row_problems(row: &Row, fixture: &BTreeMap<&str, (u8, bool)>, targets: &BTreeMap<&str, usize>) -> Vec<String> {
    let mut problems = Vec::new();
    let old = row.old.as_str();
    // The new names are registered, and the exits and retry values are the registry's.
    let mut registry_exits = Vec::new();
    let mut registry_retryable = Vec::new();
    for new in &row.new {
        match registry_row(new) {
            Some(info) => {
                registry_exits.push(info.exit);
                registry_retryable.push(info.retryable);
            }
            None => problems.push(format!("the row of `{old}` names `{new}`, which the registry lacks")),
        }
    }
    if let Some((exit, retryable)) = fixture.get(old) {
        if row.exit_before != *exit {
            problems.push(format!("the row of `{old}` says exit {} before, 0.35 had {exit}", row.exit_before));
        }
        if row.retryable_before != *retryable {
            problems.push(format!("the row of `{old}` says retryable {} before, 0.35 had {retryable}", row.retryable_before));
        }
    }
    if sorted_unique(row.exit_after.iter().copied()) != sorted_unique(registry_exits) {
        problems.push(format!("the row of `{old}` says exits {:?} after, the registry says otherwise", row.exit_after));
    }
    if sorted_unique(row.retryable_after.iter().copied()) != sorted_unique(registry_retryable) {
        problems.push(format!("the row of `{old}` says retryable {:?} after, the registry says otherwise", row.retryable_after));
    }
    // A reason is registered under one of the new codes.
    for reason in &row.reasons {
        if !row.new.iter().filter_map(|new| registry_row(new)).any(|info| info.reasons.iter().any(|r| r.reason == reason)) {
            problems.push(format!("the row of `{old}` names the reason `{reason}`, which none of its new codes registers"));
        }
    }
    // The kind says what the row shows.
    let registered = registry_row(old).is_some();
    let single = row.new.len() == 1 && row.new[0] != old;
    let kind_fits = match row.kind.as_str() {
        "kept" => row.new == [old] && registered,
        "renamed" => single && !registered,
        "merged" => single && !registered && (fixture.contains_key(row.new[0].as_str()) || targets.get(row.new[0].as_str()).is_some_and(|count| *count >= 2)),
        "split" => row.new.len() >= 2 && row.new.iter().any(|new| new == old) == registered,
        "gone" => !registered && !row.new.is_empty(),
        _ => false,
    };
    if !kind_fits {
        problems.push(format!("the row of `{old}` says `{}`, which its cells do not show", row.kind));
    }
    problems
}

/// atip.codestable.01: the table has one row for each of the 179 codes of 0.35, its new names are the registry's, and the registry's codes are all in it.
#[test]
fn atip_codestable_01_the_table_has_a_row_for_each_old_code_and_its_new_names_are_the_registrys() {
    let changelog = changelog();
    let rows = replacement_table(&changelog).expect("the replacement table reads");
    assert_eq!(rows.len(), 179, "one row for each code of 0.35");
    assert_eq!(registry_0_35().len(), 179);
    let problems = table_problems(&changelog);
    assert!(problems.is_empty(), "the replacement table is not the registry's: {problems:#?}");
    let kinds = sorted_unique(rows.iter().map(|row| row.kind.as_str()));
    assert_eq!(kinds, ["gone", "kept", "merged", "renamed", "split"]);
    // A code that was kept shows `kept`; every code the registry no longer holds is a row that says what replaced it.
    for (code, ..) in registry_0_35() {
        let row = rows.iter().find(|row| row.old == *code).unwrap();
        assert_eq!(row.kind == "kept", registry_row(code).is_some() && row.new == [*code], "{code}");
    }
    for code in gone_codes() {
        let row = rows.iter().find(|row| row.old == *code).unwrap();
        assert_ne!(row.kind, "kept", "{code} is gone from the registry");
    }
}

/// atip.codestable.02: the before and after exit, retryable and reasons of every row are the 0.35 data's and the registry's, and a row's kind says what it shows.
#[test]
fn atip_codestable_02_the_exit_retryable_and_reasons_of_every_row_are_the_registrys() {
    let rows = replacement_table(&changelog()).expect("the replacement table reads");
    // The registry's own values, row by row, are the point of the table; spot the ones a reader branches on.
    let row = |old: &str| rows.iter().find(|row| row.old == old).unwrap_or_else(|| panic!("no row for {old}"));
    assert_eq!((row("BRIEF_UNAPPROVED").exit_before, row("BRIEF_UNAPPROVED").exit_after.clone()), (1, vec![2]));
    assert_eq!((row("DISPATCH_END_IN_PROGRESS").retryable_before, row("DISPATCH_END_IN_PROGRESS").retryable_after.clone()), (true, vec![true]));
    assert_eq!(row("REPORT_STALE").reasons, ["STALE"]);
    assert_eq!(row("CONTEXT_PACK_TAMPERED").exit_after, [1, 2]);
    assert_eq!(row("STOPPED_MIDWAY").kind, "gone");
    for row in &rows {
        assert!(!row.why.is_empty(), "`{}` has no why", row.old);
    }
}

/// atip.codestable.03: a table with a row deleted, a new name misspelled, an exit wrong or an old name twice fails the table test, each fault named.
#[test]
fn atip_codestable_03_a_table_that_loses_a_row_or_misnames_or_misstates_one_fails() {
    let changelog = changelog();
    assert_eq!(table_problems(&changelog), Vec::<String>::new());
    let row_of = |old: &str| changelog.lines().find(|line| line.starts_with(&format!("| `{old}` |"))).unwrap_or_else(|| panic!("no row for {old}")).to_string();
    let human = row_of("HUMAN_ONLY");
    // A row deleted.
    let deleted = changelog.replacen(&format!("{human}\n"), "", 1);
    let after_delete = table_problems(&deleted);
    assert!(after_delete.contains(&"the table has no row for `HUMAN_ONLY`".to_string()), "{after_delete:?}");
    assert!(after_delete.iter().any(|p| p.contains("`ACTOR_DENIED` is the new name of no row")), "a code no row names is found too: {after_delete:?}");
    // A new name misspelled.
    let misspelled = changelog.replacen(&human, &human.replace("`ACTOR_DENIED`", "`ACTOR_DENID`"), 1);
    let problems = table_problems(&misspelled);
    assert!(problems.iter().any(|p| p.contains("`ACTOR_DENID`, which the registry lacks")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("`ACTOR_DENIED` is the new name of no row")), "{problems:?}");
    // An exit that is not the registry's.
    let wrong_exit = changelog.replacen(&human, &human.replace("| 2 → 2 |", "| 2 → 1 |"), 1);
    assert!(table_problems(&wrong_exit).iter().any(|p| p.contains("the row of `HUMAN_ONLY` says exits [1] after")), "{:?}", table_problems(&wrong_exit));
    // An exit before that is not 0.35's.
    let wrong_before = changelog.replacen(&human, &human.replace("| 2 → 2 |", "| 1 → 2 |"), 1);
    assert!(table_problems(&wrong_before).iter().any(|p| p.contains("says exit 1 before, 0.35 had 2")), "{:?}", table_problems(&wrong_before));
    // A retryable value that is not the registry's.
    let wrong_retry = changelog.replacen(&human, &human.replace("| no → no |", "| no → yes |"), 1);
    assert!(table_problems(&wrong_retry).iter().any(|p| p.contains("the row of `HUMAN_ONLY` says retryable [true] after")), "{:?}", table_problems(&wrong_retry));
    // An old name twice.
    let twice = changelog.replacen(&human, &format!("{human}\n{human}"), 1);
    assert_eq!(table_problems(&twice), ["`HUMAN_ONLY` has two rows"]);
    // A kind the row does not show.
    let wrong_kind = changelog.replacen(&human, &human.replace("| renamed |", "| kept |"), 1);
    assert!(table_problems(&wrong_kind).iter().any(|p| p.contains("the row of `HUMAN_ONLY` says `kept`")), "{:?}", table_problems(&wrong_kind));
    // A reason none of the new codes registers.
    let reason = row_of("REPORT_STALE");
    let wrong_reason = changelog.replacen(&reason, &reason.replace("`STALE`", "`DRIFTED`"), 1);
    assert!(table_problems(&wrong_reason).iter().any(|p| p.contains("the reason `DRIFTED`")), "{:?}", table_problems(&wrong_reason));
    // A table that is no table.
    assert_eq!(table_problems("\n## 0.36.0\n\nno table here\n\n## 0.35.0\n"), ["the Unreleased section has no replacement table"]);
}

/// The backticked code names of the bullet of `changelog` that starts with `bullet`.
fn bullet_codes(changelog: &str, bullet: &str) -> BTreeSet<String> {
    let start = changelog.find("\n## 0.36.0").expect("an Unreleased section");
    let section = &changelog[start..];
    let line = section.lines().find(|line| line.starts_with(bullet)).unwrap_or_else(|| panic!("no list that starts `{bullet}`"));
    line.split('`').skip(1).step_by(2).filter(|name| !name.starts_with('_') && name.contains('_') && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')).map(str::to_string).collect()
}

/// atip.codestable.04: the four lists under the table are its rows by exit, merge, split and removal.
#[test]
fn atip_codestable_04_the_lists_under_the_table_are_the_rows_by_exit_merge_split_and_removal() {
    let changelog = changelog();
    let rows = replacement_table(&changelog).expect("the replacement table reads");
    let pure: BTreeSet<String> = rows.iter().filter(|row| row.exit_before == 1 && row.exit_after.iter().all(|exit| *exit == 2)).map(|row| row.old.clone()).collect();
    let mixed: BTreeSet<String> = rows.iter().filter(|row| row.exit_before == 1 && row.exit_after.contains(&2) && !pure.contains(&row.old)).map(|row| row.old.clone()).collect();
    let exit_list = bullet_codes(&changelog, "- **Exit 1 → 2**");
    assert_eq!(exit_list, pure.union(&mixed).cloned().collect::<BTreeSet<_>>(), "the list of exits that moved");
    assert!(changelog.contains(&format!("- **Exit 1 → 2** ({} codes whose every new code is exit 2)", pure.len())), "the list counts {} codes", pure.len());
    let merged: BTreeSet<String> = rows.iter().filter(|row| row.kind == "merged").flat_map(|row| [row.old.clone(), row.new[0].clone()]).collect();
    assert_eq!(bullet_codes(&changelog, "- **Merged**"), merged, "the list of merged codes");
    let split: BTreeSet<String> = rows.iter().filter(|row| row.kind == "split").map(|row| row.old.clone()).collect();
    assert_eq!(bullet_codes(&changelog, "- **Split**"), split, "the list of split codes");
    let gone: BTreeSet<String> = rows.iter().filter(|row| row.kind == "gone").flat_map(|row| std::iter::once(row.old.clone()).chain(row.new.clone())).collect();
    assert_eq!(bullet_codes(&changelog, "- **Gone**"), gone, "the removed codes and what replaces them");
    // The headline of the exit-changing refusals counts the same codes.
    assert!(changelog.contains(&format!("**{} codes changed from exit 1 to exit 2", pure.len())), "the headline counts {} codes", pure.len());
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
    assert!(files.len() > 100, "the scan found {} files", files.len());
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

/// Every file a reader of canon looks in: the guides and their references, the website in both languages, the dashboard, the launcher, the eval
/// scripts, the design documents, the scenarios outside `area=atip`, the README and ARCHITECTURE; and, when `wide`, also the dashboard's tests and
/// pages, the conformance cases, the generated command documents, the justfile, the atip fixture and the sources of every crate. The CHANGELOG, the
/// history of a change (`.canon/changes`), the ledger, the artifacts and the dated research are the only places that may spell an old name; the
/// scenarios of `area=atip` name them to say they are gone.
fn consumer_files(wide: bool) -> Vec<PathBuf> {
    let root = repo_root();
    let skip = ["node_modules", "dist", "target", ".git", "__pycache__", ".astro"];
    let mut files = Vec::new();
    let text = ["md", "mdx", "ts", "tsx", "js", "mjs", "css", "html", "json", "jsonl", "py", "sh", "out", "txt", "yaml", "yml", "toml", "rs", "feature", "astro"];
    for dir in ["canon/skills", "packages/website/src", "packages/dashboard/src", "packages/cli/src", "evals", "docs/design"] {
        collect(&root.join(dir), &text, &skip, &mut files);
    }
    collect(&root.join(".canon/specs"), &["feature"], &[skip.as_slice(), &["area=atip"]].concat(), &mut files);
    files.extend(["README.md", "ARCHITECTURE.md"].iter().map(|name| root.join(name)));
    if wide {
        for dir in ["canon", "packages/cli", "packages/dashboard", "conformance", "docs/cli"] {
            collect(&root.join(dir), &text, &skip, &mut files);
        }
        for entry in std::fs::read_dir(root.join("crates")).unwrap().flatten() {
            collect(&entry.path().join("src"), &["rs"], &skip, &mut files);
        }
        files.extend(["crates/canon-cli/tests/atip/canon.fixture.json", "justfile"].iter().map(|name| root.join(name)));
        files.sort();
        files.dedup();
    }
    files.retain(|file| file.exists());
    files
}

/// atip.codestable.07: no consumer spells a code of 0.35 that the registry no longer holds.
#[test]
fn atip_codestable_07_no_consumer_spells_a_gone_code() {
    let files = consumer_files(true);
    assert!(files.len() > 300, "the scan found {} files: a consumer directory moved", files.len());
    assert!(gone_codes().len() > 100, "the 0.35 data holds the gone codes: {}", gone_codes().len());
    for directory in ["canon/skills", "packages/website/src/content/docs/ko", "packages/website/src/content/docs/concepts", "packages/dashboard/src", "evals", "conformance/cases", "docs/design", ".canon/specs"] {
        assert!(files.iter().any(|file| file.strip_prefix(repo_root()).is_ok_and(|rel| rel.starts_with(directory))), "the scan reaches {directory}");
    }
    let hits: Vec<String> = files
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok().map(|text| (file, text)))
        .flat_map(|(file, text)| gone_codes_in(&text).into_iter().map(move |code| format!("{} spells `{code}`", file.strip_prefix(repo_root()).unwrap_or(file).display())))
        .collect();
    assert!(hits.is_empty(), "gone codes left in consumers: {hits:#?}");
}

/// atip.codestable.08: a text that revives a gone code or says a code the registry lacks is named by the guard; a registered code passes.
#[test]
fn atip_codestable_08_a_revived_or_misspelled_code_is_named() {
    let names = registered_names();
    assert_eq!(gone_codes_in("the guide still says HUMAN_ONLY and NO_COMMIT"), ["HUMAN_ONLY", "NO_COMMIT"]);
    assert_eq!(gone_codes_in("STOPPED_MIDWAY is gone and MY_STOPPED_MIDWAY_TOO is another name"), ["STOPPED_MIDWAY"]);
    assert_eq!(gone_codes_in("a guide that says ACTOR_DENIED and BRIEF_CHECK_FAILED"), Vec::<&str>::new());
    assert_eq!(gone_codes_in("CONFIG_MISSING stays the name of a warning, and LAYOUT_MIGRATION_REQUIRED of an error"), Vec::<&str>::new());
    assert_eq!(unregistered_codes_in("refused with `ACTOR_DENID`", &names), ["ACTOR_DENID"], "a misspelled new name");
    assert_eq!(unregistered_codes_in("refused with `ACTOR_FORBIDDEN_INVALID`", &names), ["ACTOR_FORBIDDEN_INVALID"], "a name canon never had");
    assert_eq!(unregistered_codes_in("refused with `ACTOR_DENIED`", &names), Vec::<String>::new());
    // The real consumers say only codes canon registers.
    let hits: Vec<String> = consumer_files(false)
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok().map(|text| (file, text)))
        .flat_map(|(file, text)| unregistered_codes_in(&text, &names).into_iter().map(move |code| format!("{} says `{code}`, which canon does not register", file.strip_prefix(repo_root()).unwrap_or(file).display())))
        .collect();
    assert!(hits.is_empty(), "consumers say codes canon does not have: {hits:#?}");
}

/// The registered error codes a page names.
fn codes_named(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut word = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            if registry_row(&word).is_some() {
                found.insert(word.clone());
            }
            word.clear();
        }
    }
    found
}

/// atip.codestable.09: each English page of the website and its Korean page name the same registered codes.
#[test]
fn atip_codestable_09_the_website_says_the_same_codes_in_english_and_korean() {
    let docs = repo_root().join("packages/website/src/content/docs");
    let mut pages = Vec::new();
    collect(&docs, &["md", "mdx"], &["ko"], &mut pages);
    assert!(pages.len() >= 8, "the English pages: {}", pages.len());
    let mut with_codes = 0;
    for english in &pages {
        let relative = english.strip_prefix(&docs).unwrap();
        let korean = docs.join("ko").join(relative);
        assert!(korean.exists(), "{} has no Korean page", relative.display());
        let (en, ko) = (codes_named(&std::fs::read_to_string(english).unwrap()), codes_named(&std::fs::read_to_string(&korean).unwrap()));
        assert_eq!(en, ko, "{}: the English and the Korean page name different codes", relative.display());
        with_codes += usize::from(!en.is_empty());
    }
    assert!(with_codes >= 1, "some page names a code");
    // A pair that disagrees is found.
    assert_ne!(codes_named("refused with ACTOR_DENIED"), codes_named("refused with ROLE_INVALID"));
}

/// The exit-1 sentences of `text` that name a refusal code beside `exit 1`: `(line, code)`. A code of the registry whose only exit is 2 is a refusal.
fn exit_one_refusals(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for line in text.lines() {
        for (at, _) in line.match_indices("exit") {
            let rest = &line[at..];
            let after = rest.strip_prefix("exits").or_else(|| rest.strip_prefix("exit")).unwrap_or(rest).trim_start();
            if !after.starts_with('1') || after[1..].starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            // The codes the exit belongs to: those after it up to the next `exit` (80 bytes at most), and those just before it (40 bytes) when no other
            // `exit` stands there; both on char boundaries.
            let mut from = at.saturating_sub(40);
            while !line.is_char_boundary(from) {
                from += 1;
            }
            if line[from..at].contains("exit") {
                from = at;
            }
            let mut to = line[at + 4..].find("exit").map_or(line.len(), |p| at + 4 + p).min(at + 80);
            while !line.is_char_boundary(to) {
                to -= 1;
            }
            for code in codes_named(&line[from..to]) {
                if registry_row(&code).is_some_and(|info| info.exit == 2) {
                    found.push((line.to_string(), code));
                }
            }
        }
    }
    found
}

/// atip.codestable.10: no guide, design document, page or help says exit 1 for a refusal of a code the registry makes exit 2.
#[test]
fn atip_codestable_10_no_guide_says_exit_1_for_a_refusal() {
    assert_eq!(exit_one_refusals("a refused gate is exit 1 with `SUBJECT_INVALID`").len(), 1);
    assert_eq!(exit_one_refusals("a tampered pack is `CONTEXT_PACK_CORRUPT`, exit 1").len(), 1);
    assert!(exit_one_refusals("`gate check` exits 1 with `GATE_CHECK_FAILED`; a refusal is exit 2 with `SUBJECT_INVALID`").is_empty());
    assert!(exit_one_refusals("exit 1 = violations; exit 2 = `CONFIG_REQUIRED`").is_empty());
    let root = repo_root();
    let skip = ["node_modules", "dist", "target", ".git", "__pycache__", ".astro"];
    let mut files = Vec::new();
    for dir in ["canon/skills", "packages/website/src", "packages/dashboard/src", "evals", "docs/design", "docs/cli"] {
        collect(&root.join(dir), &["md", "mdx", "ts", "tsx", "py", "json"], &skip, &mut files);
    }
    files.extend(["README.md", "ARCHITECTURE.md"].iter().map(|name| root.join(name)));
    collect(&root.join("crates/canon-cli/src/cli"), &["rs"], &skip, &mut files);
    let hits: Vec<String> = files
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok().map(|text| (file, text)))
        .flat_map(|(file, text)| exit_one_refusals(&text).into_iter().map(move |(_, code)| format!("{} says exit 1 beside `{code}`, which is exit 2", file.strip_prefix(repo_root()).unwrap_or(file).display())))
        .collect();
    assert!(hits.is_empty(), "a refusal said as exit 1: {hits:#?}");
}

/// The `## 0.36.0` section of the CHANGELOG.
fn unreleased(changelog: &str) -> &str {
    let start = changelog.find("\n## 0.36.0").expect("an Unreleased section");
    let rest = &changelog[start + 1..];
    &rest[..rest[3..].find("\n## ").map_or(rest.len(), |at| at + 3)]
}

/// The table of changed scenarios kept beside the change: its header counts the rows, and each row is `| @<id> | <file> | <kind> | <what changed> |`.
const SCENARIO_DIFF: &str = ".canon/changes/canon-0-36-codes-table/scenario-diff.md";

/// What is wrong with `diff` (the table of changed scenarios) against `scenarios` (every scenario tag of the specs): the count, the ids, the kinds.
fn diff_problems(diff: &str, scenarios: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    let declared: Option<usize> = diff.lines().find_map(|line| line.strip_prefix("Scenarios changed: ")).and_then(|n| n.trim().parse().ok());
    let rows: Vec<Vec<&str>> = diff.lines().filter(|line| line.starts_with("| @")).map(|line| line.split('|').map(str::trim).collect()).collect();
    if declared != Some(rows.len()) {
        problems.push(format!("the header counts {declared:?} scenarios, the table has {} rows", rows.len()));
    }
    let mut seen = BTreeSet::new();
    for cells in &rows {
        let [_, id, _file, kind, what, _] = cells.as_slice() else {
            problems.push(format!("a row does not have four cells: {cells:?}"));
            continue;
        };
        if !seen.insert(*id) {
            problems.push(format!("{id} has two rows"));
        }
        if !scenarios.contains(*id) {
            problems.push(format!("{id} is no scenario of the specs"));
        }
        if !["name only", "meaning"].contains(kind) || what.is_empty() {
            problems.push(format!("{id} has the kind `{kind}` and the change `{what}`"));
        }
    }
    problems
}

/// Every scenario tag (`@area.surface.nn`) of the `.feature` files under `.canon/specs`.
fn scenario_tags() -> BTreeSet<String> {
    let mut files = Vec::new();
    collect(&repo_root().join(".canon/specs"), &["feature"], &[], &mut files);
    files
        .iter()
        .flat_map(|file| std::fs::read_to_string(file).unwrap().lines().filter_map(|line| line.trim().strip_prefix('@').filter(|tag| !tag.contains(':') && tag.matches('.').count() == 2).map(|tag| format!("@{tag}"))).collect::<Vec<_>>())
        .collect()
}

/// atip.codestable.11: the table of changed scenarios has one row for each, names each once, and says whether only a code name changed.
#[test]
fn atip_codestable_11_the_table_of_changed_scenarios_has_a_row_for_each_and_names_the_kind() {
    let diff = std::fs::read_to_string(repo_root().join(SCENARIO_DIFF)).expect("the table of changed scenarios");
    let scenarios = scenario_tags();
    assert!(scenarios.len() > 1000, "the specs hold {} scenarios", scenarios.len());
    assert_eq!(diff_problems(&diff, &scenarios), Vec::<String>::new());
    let rows = diff.lines().filter(|line| line.starts_with("| @")).count();
    assert!(rows >= 100, "the scenarios 0.36 changed: {rows}");
    // A table that miscounts, repeats a scenario, names one that does not exist or leaves a kind out is found.
    let first = diff.lines().find(|line| line.starts_with("| @")).unwrap();
    let id = first.split('|').nth(1).unwrap().trim();
    assert!(diff_problems(&diff.replacen(first, "", 1), &scenarios).iter().any(|p| p.contains("the header counts")));
    assert!(diff_problems(&diff.replacen(first, &format!("{first}\n{first}"), 1), &scenarios).iter().any(|p| p.contains(&format!("{id} has two rows"))));
    assert!(diff_problems(&diff.replacen(id, "@no.such.99", 1), &scenarios).iter().any(|p| p.contains("@no.such.99 is no scenario")));
    assert!(diff_problems(&diff.replacen("| name only |", "| typo only |", 1), &scenarios).iter().any(|p| p.contains("typo only")));
}

/// What is wrong with the upgrade note of `changelog`: it speaks to the four callers, names each exit that moved and each command whose refusal
/// carries details, says `STOPPED_MIDWAY` is gone and says there is no alias.
fn note_problems(changelog: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let section = unreleased(changelog);
    let Some(note_start) = section.find("**Upgrade note: a script, hook, guide or dashboard") else { return vec!["the Unreleased section has no upgrade note".to_string()] };
    let note = &section[note_start..section[note_start..].find("\n\n**").map_or(section.len(), |at| note_start + at)];
    for caller in ["- **Scripts and CI that read `error.code`.**", "- **Scripts and hooks that read the exit.**", "- **Agent guides and prompts.**", "- **The dashboard and what reads its API.**"] {
        if !note.contains(caller) {
            problems.push(format!("the note has no entry `{caller}`"));
        }
    }
    // Every code whose exit moved from 1 to 2 is named, in the table's list and in the note's own words by command.
    let rows = replacement_table(changelog).expect("the replacement table reads");
    let moved: Vec<&str> = rows.iter().filter(|row| row.exit_before == 1 && row.exit_after.contains(&2)).map(|row| row.old.as_str()).collect();
    if moved.len() < 26 {
        problems.push(format!("the codes whose exit moved: {}", moved.len()));
    }
    let list = bullet_codes(changelog, "- **Exit 1 → 2**");
    for code in &moved {
        if !list.contains(*code) {
            problems.push(format!("the list of moved exits lacks {code}"));
        }
    }
    let pure = rows.iter().filter(|row| row.exit_before == 1 && row.exit_after.iter().all(|exit| *exit == 2)).count();
    if !(note.contains("the first list above") && note.contains(&format!("The {pure} codes"))) {
        problems.push("the note does not count the moved exits".to_string());
    }
    for command in ["subject move", "gate promote", "task done", "evidence add", "brief approve", "strategy promote", "strategy approval-payload", "ingest plans", "dispatch begin", "dispatch end", "divergence defer", "divergence resolve", "dashboard", "context-pack add"] {
        if !note.contains(&format!("`{command}`")) {
            problems.push(format!("the note does not name `{command}` among the commands whose exit moved"));
        }
    }
    // The refusals that carry `error.details` instead of `data`.
    match note.find("- **Refusals carry `error.details`, not `data`.**") {
        None => problems.push("the note has no entry on details".to_string()),
        Some(at) => {
            let details = &note[at..];
            let details = &details[..details.find("\n- **").unwrap_or(details.len())];
            for command in ["gate promote", "subject move", "task done", "brief approve"] {
                if !details.contains(&format!("`{command}")) {
                    problems.push(format!("the entry on details does not name `{command}`"));
                }
            }
            if !(details.contains("`error.details[]`") && details.contains("`data.refused`")) {
                problems.push("the entry on details does not name `error.details[]` and `data.refused`".to_string());
            }
        }
    }
    if !note.contains("`STOPPED_MIDWAY`, which is gone") {
        problems.push("the note does not say the reserved code is gone".to_string());
    }
    if !note.contains("There is no alias") {
        problems.push("the note does not say there is no alias".to_string());
    }
    problems
}

/// atip.codestable.12: the upgrade note speaks to the four callers and names each exit that moved and each command whose refusal carries details.
#[test]
fn atip_codestable_12_the_upgrade_note_covers_the_four_callers_the_moved_exits_and_the_details_commands() {
    assert_eq!(note_problems(&changelog()), Vec::<String>::new());
}

/// atip.codestable.15 (the failure path of F5): an upgrade note that leaves a caller, a moved code, a command that carries details, the gone code or the
/// word on aliases out fails the note check with the missing part named, and a CHANGELOG with no note fails it too.
#[test]
fn atip_codestable_15_an_upgrade_note_that_leaves_a_caller_a_moved_exit_or_a_command_out_fails() {
    let changelog = changelog();
    let broken = |from: &str, to: &str| {
        assert!(changelog.contains(from), "the CHANGELOG has no `{from}` to break");
        note_problems(&changelog.replacen(from, to, 1))
    };
    assert!(broken("- **The dashboard and what reads its API.**", "- **Dashboards.**").contains(&"the note has no entry `- **The dashboard and what reads its API.**`".to_string()));
    assert!(broken("- **Agent guides and prompts.**", "- **Prompts.**").iter().any(|p| p.contains("Agent guides and prompts")));
    assert!(broken("There is no alias", "There may be an alias").contains(&"the note does not say there is no alias".to_string()));
    assert!(broken("`STOPPED_MIDWAY`, which is gone", "`STOPPED_MIDWAY`, which stays").contains(&"the note does not say the reserved code is gone".to_string()));
    assert!(broken("- **Refusals carry `error.details`, not `data`.**", "- **Refusals carry details.**").contains(&"the note has no entry on details".to_string()));
    let without_note = changelog.replacen("**Upgrade note: a script, hook, guide or dashboard", "**Release note", 1);
    assert_eq!(note_problems(&without_note), ["the Unreleased section has no upgrade note"]);
    // A moved code missing from the list under the table, and a command the note stopped naming.
    let list_start = changelog.find("- **Exit 1 → 2**").expect("the list of moved exits");
    let first_code = changelog[list_start..].split('`').nth(1).expect("a code of the list").to_string();
    let mut without_code = changelog.clone();
    let line_end = list_start + changelog[list_start..].find('\n').unwrap();
    without_code.replace_range(list_start..line_end, &changelog[list_start..line_end].replacen(&format!("`{first_code}`"), "", 1));
    assert!(note_problems(&without_code).iter().any(|p| p.contains(&format!("lacks {first_code}"))), "{:?}", note_problems(&without_code));
    let note_start = changelog.find("**Upgrade note: a script, hook, guide or dashboard").unwrap();
    let mut without_command = changelog.clone();
    let note_end = note_start + changelog[note_start..].find("\n\n**").unwrap_or(changelog.len() - note_start);
    without_command.replace_range(note_start..note_end, &changelog[note_start..note_end].replace("`dispatch begin`", "dispatch begin"));
    assert!(note_problems(&without_command).iter().any(|p| p.contains("`dispatch begin`")), "{:?}", note_problems(&without_command));
}

/// The lines of `section` that name a gone code (`gone_codes_in`) as if it were current: not a row of a table (the old-name column), not the
/// lists under the table, and not a sentence that says the name is gone, replaced, renamed or 0.35's.
fn current_gone_code_lines(section: &str) -> Vec<String> {
    const SAYS_GONE: [&str; 12] = ["gone", "0.35", "replace", "renamed", "left the registry", "no longer", "removed", "retired", "a script", "a branch on", "a retry loop", "does not list"];
    section
        .lines()
        .filter(|line| !line.starts_with('|'))
        .filter(|line| !["- **Gone**", "- **Exit 1", "- **Merge", "- **Split", "- **Renamed", "- **Removed"].iter().any(|bullet| line.starts_with(bullet)))
        .flat_map(|line| line.split(". ").map(str::to_string).collect::<Vec<_>>())
        .filter(|sentence| !SAYS_GONE.iter().any(|word| sentence.to_lowercase().contains(word)))
        .filter_map(|sentence| {
            let codes = gone_codes_in(&sentence);
            (!codes.is_empty()).then(|| format!("{codes:?}: {}", sentence.chars().take(160).collect::<String>()))
        })
        .collect()
}

/// atip.codestable.13: in the Unreleased section, outside the old-name column of the table and the lists under it, no paragraph names a removed code as
/// current behaviour (`STOPPED_MIDWAY` was described as live by two stage paragraphs after the codes were replaced); the guard finds one that does.
#[test]
fn atip_codestable_13_no_paragraph_names_a_removed_code_as_current() {
    let changelog = changelog();
    let problems = current_gone_code_lines(unreleased(&changelog));
    assert!(problems.is_empty(), "paragraphs of the Unreleased section name a removed code as current: {problems:#?}");
    // The guard can fail: a stage paragraph that still says the old code, and one that says it is gone.
    let stale = "A command whose promotion the gate refused is `STOPPED_MIDWAY` (exit 1, reason `PROMOTION_INVALID`).";
    assert_eq!(current_gone_code_lines(stale).len(), 1, "a removed code named as live is found");
    assert!(current_gone_code_lines("`STOPPED_MIDWAY` is gone: the `_INCOMPLETE` codes replace it.").is_empty());
    assert!(current_gone_code_lines("| `STOPPED_MIDWAY` | `INIT_INCOMPLETE` | split |").is_empty(), "the old-name column of the table is the one place");
}

/// atip.codestable.14 (the failure path of F4): a table of changed scenarios that miscounts its rows, repeats a scenario, names one the specs lack, leaves
/// the kind out or the change unsaid, or has a row with a missing cell is found, so a table cannot stand for reviews it does not list; and a table
/// with a scenario of the specs dropped is found by the count.
#[test]
fn atip_codestable_14_a_table_of_changed_scenarios_that_miscounts_or_misnames_a_row_is_found() {
    let diff = std::fs::read_to_string(repo_root().join(SCENARIO_DIFF)).expect("the table of changed scenarios");
    let scenarios = scenario_tags();
    assert_eq!(diff_problems(&diff, &scenarios), Vec::<String>::new());
    let first = diff.lines().find(|line| line.starts_with("| @")).unwrap();
    let cells: Vec<&str> = first.split('|').map(str::trim).collect();
    let (id, kind, what) = (cells[1], cells[3], cells[4]);
    // A row dropped: the header counts one more than the table has.
    let dropped = diff_problems(&diff.replacen(first, "", 1), &scenarios);
    assert!(dropped.iter().any(|p| p.starts_with("the header counts")), "{dropped:?}");
    // A header that counts one row too many or too few.
    let header = diff.lines().find(|line| line.starts_with("Scenarios changed: ")).unwrap();
    let declared: usize = header.trim_start_matches("Scenarios changed: ").trim().parse().unwrap();
    let overcount = diff_problems(&diff.replacen(header, &format!("Scenarios changed: {}", declared + 1), 1), &scenarios);
    assert!(overcount.iter().any(|p| p.starts_with("the header counts")), "{overcount:?}");
    // A scenario twice, a scenario the specs lack, a kind not in the vocabulary, a change left unsaid, and a row short of a cell.
    assert_eq!(diff_problems(&diff.replacen(first, &format!("{first}\n{first}"), 1), &scenarios).iter().filter(|p| p.contains(&format!("{id} has two rows"))).count(), 1);
    assert!(diff_problems(&diff.replacen(id, "@no.such.99", 1), &scenarios).iter().any(|p| p.contains("@no.such.99 is no scenario")));
    assert!(diff_problems(&diff.replacen(first, &first.replacen(&format!("| {kind} |"), "| |", 1), 1), &scenarios).iter().any(|p| p.contains(id)));
    assert!(diff_problems(&diff.replacen(first, &first.replacen(&format!("| {what} |"), "| |", 1), 1), &scenarios).iter().any(|p| p.contains(id)));
    let short = first.replacen(&format!(" {} |", cells[2]), "", 1);
    assert!(diff_problems(&diff.replacen(first, &short, 1), &scenarios).iter().any(|p| p.contains("does not have four cells")), "a row short of a cell goes unseen");
    // A scenario of the specs missing from the specs' tag set: the row that names it is found.
    let mut fewer = scenarios.clone();
    fewer.remove(id);
    assert!(diff_problems(&diff, &fewer).iter().any(|p| p.contains(&format!("{id} is no scenario of the specs"))));
}
