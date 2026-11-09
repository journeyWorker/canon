//! `canon-0-40-ci-evidence` F7: the people who read the skills and the CHANGELOG find the recipe, the new field and the new policy key,
//! and a generated block that no longer matches the command spec fails the drift check (`tests/docs_generated.rs`).

#[expect(dead_code, reason = "this test uses only the splice half of the generator")]
mod doc_gen;

use std::fs;

fn read(relative: &str) -> String {
    fs::read_to_string(doc_gen::repo_root().join(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

/// The CHANGELOG's `## Unreleased` section.
fn unreleased() -> String {
    let changelog = read("CHANGELOG.md");
    let start = changelog.find("## Unreleased").expect("an Unreleased section");
    let rest = &changelog[start + "## Unreleased".len()..];
    rest[..rest.find("\n## ").unwrap_or(rest.len())].to_string()
}

/// gate.ci-run.13 (F7): the GitHub Actions recipe follows "One report, many scenarios", the policy and subject skills name the key and the
/// check, the schema has the field, and the CHANGELOG lists what was added.
#[test]
fn gate_ci_run_13_the_skills_and_the_changelog_carry_the_recipe_the_field_and_the_key() {
    let gate = read("canon/skills/reference/canon-gate.md");
    let one_report = gate.find("### One report, many scenarios").expect("the section the recipe follows");
    let recipe = gate.find("### Recording the suite from CI").expect("the GitHub Actions recipe");
    assert!(recipe > one_report, "the recipe comes after `One report, many scenarios`");
    let recipe = &gate[recipe..];
    let recipe = &recipe[..recipe[4..].find("\n### ").map_or(recipe.len(), |end| end + 4)];
    for needle in [
        "fetch-depth: 0",
        "if: always()",
        "canon inventory sync",
        "canon evidence add --project root --all-scenarios --from-ci",
        "--report junit:target/nextest/ci/junit.xml",
        "--role implementer --promote",
        "actions/upload-artifact",
        "permissions",
        "GITHUB_TOKEN",
        ".canon/ledger",
        ".canon/artifacts",
        "CI_ENV_REQUIRED",
        "CI_WORKTREE_CONFLICT",
    ] {
        assert!(recipe.contains(needle), "the recipe lacks `{needle}`");
    }
    let order: Vec<usize> = ["cargo nextest run", "canon inventory sync", "actions/upload-artifact"].iter().map(|step| recipe.find(step).unwrap_or_else(|| panic!("no `{step}`"))).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "tests, then the record, then the upload: {order:?}");

    let policy = read("canon/skills/reference/canon-policy.md");
    for needle in ["spec_coverage.require_ci_run", "min_cases", "no_ci_run", "other_code_tree", "dirty_worktree", "short_run", "code_tree_unknown", "--override-reason"] {
        assert!(policy.contains(needle), "canon-policy lacks `{needle}`");
    }
    let subject = read("canon/skills/reference/canon-subject.md");
    assert!(subject.contains("require_ci_run") && subject.contains("`--override-reason` does not waive it"), "canon-subject names the guard's check");

    let schema = read("crates/canon-model/schemas/evidence.schema.json");
    assert!(schema.contains("\"ci\"") && schema.contains("\"CiRun\"") && schema.contains("\"attempt\""), "the evidence schema has the `ci` block");

    let added = unreleased();
    for needle in ["--from-ci", "`ci` block", "data.reports[]", "spec_coverage.require_ci_run", "CI_ENV_REQUIRED", "CI_WORKTREE_CONFLICT", "a claim the environment makes"] {
        assert!(added.contains(needle), "the CHANGELOG's Unreleased section lacks `{needle}`");
    }
    assert!(read("canon/skills/reference/role-implementer.md").contains("--from-ci"), "the generated command block of the implementer's guide lists --from-ci");
}

/// gate.ci-run.14 (F7, failure path): a generated block edited by hand, or left behind by a spec change, is not what the spec renders, and
/// the drift check names the difference.
#[test]
fn gate_ci_run_14_a_generated_block_that_does_not_match_the_spec_is_drift() {
    let spec = canon_cli::spec::model();
    let text = read("canon/skills/reference/role-implementer.md");
    assert_eq!(doc_gen::splice(&text, false, false, &spec).unwrap(), text, "the committed block is what the spec renders");

    let stale = text.replacen("--from-ci", "--from-cx", 1);
    assert_ne!(stale, text, "the block lists --from-ci");
    let rendered = doc_gen::splice(&stale, false, false, &spec).unwrap();
    assert_ne!(rendered, stale, "the stale block is drift");
    assert_eq!(rendered, text, "the render restores it");
}
