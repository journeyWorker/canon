//! `canon-0-39-secret-scan`, F6: the release check. The CHANGELOG entry of the change says what a reader of the release has to know: the refusals
//! that stop, the `dirty` that was wrong, the inputs that are refused now, and that the pack id of the same input changed.

use std::path::Path;

/// What the CHANGELOG lacks of the announcement; each missing item is named. `changelog` is the whole file.
fn release_problems(changelog: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(section) = changelog.split("\n## ").find(|section| section.contains("canon-0-39-secret-scan")) else {
        return vec!["no release section of the CHANGELOG names `canon-0-39-secret-scan`".to_string()];
    };
    let part = |name: &str| section.split("\n### ").find(|part| part.starts_with(name));
    match part("Fixed") {
        None => problems.push("the section has no Fixed part".to_string()),
        Some(fixed) => {
            for needle in ["risk-high", "`dirty: false`"] {
                if !fixed.contains(needle) {
                    problems.push(format!("the Fixed entry never names `{needle}`"));
                }
            }
        }
    }
    match part("Changed") {
        None => problems.push("the section has no Changed part".to_string()),
        Some(changed) => {
            for needle in ["api_key =", "JSON", "sk-", "pack id", "stateDigest"] {
                if !changed.contains(needle) {
                    problems.push(format!("the Changed entry never names `{needle}`"));
                }
            }
        }
    }
    problems
}

fn changelog() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap()
}

#[test]
fn the_changelog_announces_the_scanner_the_fixed_refusals_and_the_new_pack_ids() {
    assert_eq!(release_problems(&changelog()), Vec::<String>::new());
}

#[test]
fn a_release_without_the_announcement_fails_the_release_check() {
    let real = changelog();
    assert_eq!(release_problems("# Changelog\n\n## Unreleased\n\n## 0.38.0\n"), ["no release section of the CHANGELOG names `canon-0-39-secret-scan`"]);
    let without = real.replace("risk-high", "removed");
    assert_eq!(release_problems(&without), ["the Fixed entry never names `risk-high`"]);
    let without = real.replace("stateDigest", "removed");
    assert_eq!(release_problems(&without), ["the Changed entry never names `stateDigest`"]);
}
