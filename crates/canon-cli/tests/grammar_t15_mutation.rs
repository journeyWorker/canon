//! T15 (0.28): seeded violations each fail the right test.
//!
//! The grammar tests (T1-T14) are only worth having if they fail when the grammar is
//! broken. This copies the workspace sources to a scratch directory, applies one
//! violation at a time (a dropped metadata row, a dropped option help, a hand-built JSON
//! print, a runnable group, an option name with another value name on one leaf, a vocabulary
//! row dropped), and runs the test that must catch it there: it has to fail, on an
//! assertion and not on a compile error, while the same test passes on the unmutated copy.
//!
//! It builds the copy (the first run compiles every workspace crate under the scratch
//! path), so it is `#[ignore]`d. Run it with `scripts/check-mutations.sh`, or
//! `cargo test -p canon-cli --test grammar_t15_mutation -- --ignored --nocapture`;
//! CI runs it as its own job. `CARGO_TARGET_DIR` (default: this repo's `target`)
//! holds the dependencies already built, which the copy reuses.

mod harness;

use std::path::{Path, PathBuf};
use std::process::Command;

use harness::*;

/// What to copy: the workspace as `cargo` and the tests see it.
const COPIED: &[&str] = &["Cargo.toml", "Cargo.lock", ".cargo", "crates", "canon", "docs/design", "docs/cli"];

/// One seeded violation: which file it edits, how, and which test must fail.
struct Mutation {
    name: &'static str,
    file: &'static str,
    apply: fn(&str) -> String,
    /// The test binary and the test (`--exact`) that has to fail.
    catches: (&'static str, &'static str),
}

fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the mutation's anchor `{from}` is gone: update tests/grammar_t15_mutation.rs");
    text.replacen(from, to, 1)
}

/// The text with the first line that satisfies `pick` removed.
fn drop_line(text: &str, pick: impl Fn(&str) -> bool) -> String {
    let at = text.lines().position(pick).expect("the mutation's anchor line is gone: update tests/grammar_t15_mutation.rs");
    text.lines().enumerate().filter(|(i, _)| *i != at).map(|(_, l)| format!("{l}\n")).collect()
}

const MUTATIONS: &[Mutation] = &[
    Mutation {
        name: "drop a metadata row",
        file: "crates/canon-cli/src/schema.rs",
        apply: |text| drop_line(text, |l| l.trim_start().starts_with("row_exits(\"brief show\"")),
        catches: ("cli_tree", "every_leaf_has_one_metadata_row_and_every_row_a_leaf"),
    },
    Mutation {
        name: "drop an option's help",
        file: "crates/canon-cli/src/cli/top.rs",
        apply: |text| {
            let at = text.find("pub struct PurgeArgs").expect("PurgeArgs");
            let (head, tail) = text.split_at(at);
            let doc = tail.lines().position(|l| l.trim_start().starts_with("///")).expect("a doc comment on a PurgeArgs field");
            let tail: String = tail.lines().enumerate().filter(|(i, _)| *i != doc).map(|(_, l)| format!("{l}\n")).collect();
            format!("{head}{tail}")
        },
        catches: ("grammar_t3_options", "every_option_says_what_it_is"),
    },
    Mutation {
        name: "print JSON built by hand",
        file: "crates/canon-cli/src/status.rs",
        apply: |text| {
            let at = text.lines().position(|l| l.starts_with("pub fn ") || l.starts_with("fn ")).expect("a function to insert before");
            let mut out = String::new();
            for (i, line) in text.lines().enumerate() {
                if i == at {
                    out.push_str("#[allow(dead_code)]\nfn seeded_leak() {\n    println!(\"{}\", serde_json::json!({ \"leak\": true }));\n}\n");
                }
                out.push_str(line);
                out.push('\n');
            }
            out
        },
        catches: ("grammar_t10_typing", "no_source_prints_json_it_built_by_hand"),
    },
    Mutation {
        name: "make a group runnable",
        file: "crates/canon-cli/src/cli/mod.rs",
        apply: |text| replace_once(text, "    Gate {\n        #[command(subcommand)]", "    #[command(subcommand_required = false, arg_required_else_help = false)]\n    Gate {\n        #[command(subcommand)]"),
        catches: ("cli_tree", "no_group_is_runnable"),
    },
    Mutation {
        name: "give an option name another value name on one leaf",
        file: "crates/canon-cli/src/cli/divergence.rs",
        apply: |text| replace_once(text, "#[arg(long, value_name = \"ACTOR_ID\")]", "#[arg(long, value_name = \"NAME\")]"),
        catches: ("grammar_t3_options", "one_name_has_one_value_name_arity_and_value_set_on_every_leaf"),
    },
    Mutation {
        name: "drop a row of the vocabulary",
        file: "docs/design/cli-vocabulary.tsv",
        apply: |text| drop_line(text, |l| l.starts_with("actor-id\t")),
        catches: ("grammar_t3_options", "the_vocabulary_and_the_spec_use_the_same_options_with_the_same_value_names"),
    },
];

fn copy_into(from: &Path, to: &Path) {
    if from.is_dir() {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == "target" {
                continue;
            }
            copy_into(&entry.path(), &to.join(entry.file_name()));
        }
    } else {
        std::fs::copy(from, to).unwrap();
    }
}

/// `cargo test` in the copy for the given (binary, test) pairs; returns (success, combined output).
fn cargo_test(work: &Path, target: &Path, tests: &[(&str, &str)]) -> (bool, String) {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.args(["test", "--offline", "-p", "canon-cli", "--no-fail-fast"]);
    let mut binaries: Vec<&str> = tests.iter().map(|(binary, _)| *binary).collect();
    binaries.sort_unstable();
    binaries.dedup();
    for binary in &binaries {
        command.args(["--test", binary]);
    }
    command.args(["--", "--exact"]);
    for (_, test) in tests {
        command.arg(test);
    }
    let out = command.current_dir(work).env("CARGO_TARGET_DIR", target).env_remove("CANON_WITNESS_DIR").output().expect("spawn cargo");
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
#[ignore = "builds a copy of the workspace; run scripts/check-mutations.sh"]
fn each_seeded_violation_fails_the_test_that_guards_it() {
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| repo_root().join("target"));
    let scratch = tempfile::tempdir().unwrap();
    let work = scratch.path().join("repo");
    std::fs::create_dir_all(&work).unwrap();
    for item in COPIED {
        let from = repo_root().join(item);
        assert!(from.exists(), "{item} is part of the workspace the mutation copies");
        std::fs::create_dir_all(work.join(item).parent().unwrap()).unwrap();
        copy_into(&from, &work.join(item));
    }

    // The unmutated copy passes every guarding test, so a failure below is the mutation's.
    let guards: Vec<(&str, &str)> = MUTATIONS.iter().map(|m| m.catches).collect();
    let (passed, output) = cargo_test(&work, &target, &guards);
    assert!(passed, "the unmutated copy must pass its guards:\n{output}");

    let mut problems = Vec::new();
    for mutation in MUTATIONS {
        let path = work.join(mutation.file);
        let original = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, (mutation.apply)(&original)).unwrap();
        let (passed, output) = cargo_test(&work, &target, &[mutation.catches]);
        std::fs::write(&path, original).unwrap();
        let (binary, test) = mutation.catches;
        if passed {
            problems.push(format!("`{}`: {binary}::{test} still passes: nothing catches it", mutation.name));
        } else if output.contains("could not compile") || output.contains("error[E") {
            problems.push(format!("`{}`: the mutated copy does not compile, so it proves nothing:\n{output}", mutation.name));
        } else if !output.contains(&format!("{test} ... FAILED")) {
            problems.push(format!("`{}`: it failed, but not in {binary}::{test}:\n{output}", mutation.name));
        } else {
            eprintln!("caught: {} by {binary}::{test}", mutation.name);
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
