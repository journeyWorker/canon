//! The clap tree's own invariants: no group is runnable, and every leaf and
//! only leaves have a metadata row.

use canon_cli::cli::{self, meta::meta, Cli};
use canon_cli::schema::{self, META};
use clap::Parser;

#[test]
fn the_clap_tree_is_well_formed() {
    cli::command().debug_assert();
}

/// A group (a command with subcommands) must demand one, so a bare group is a
/// usage error and never a runnable command.
#[test]
fn no_group_is_runnable() {
    fn walk(cmd: &clap::Command, path: &mut Vec<String>, groups: &mut usize, problems: &mut Vec<String>) {
        let subs: Vec<&clap::Command> = cmd.get_subcommands().filter(|c| c.get_name() != "help").collect();
        if !subs.is_empty() {
            *groups += 1;
            if !cmd.is_subcommand_required_set() {
                problems.push(format!("`{}` is a group that is not subcommand_required", path.join(" ")));
            }
        }
        for sub in subs {
            path.push(sub.get_name().to_string());
            walk(sub, path, groups, problems);
            path.pop();
        }
    }
    let mut root = cli::command();
    root.build();
    let (mut groups, mut problems) = (0, Vec::new());
    walk(&root, &mut vec!["canon".into()], &mut groups, &mut problems);
    assert!(groups > 20, "the walk found only {groups} groups");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_leaf_has_one_metadata_row_and_every_row_a_leaf() {
    let problems = schema::check_metadata(&cli::command(), META);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    let examples = schema::check_examples(&cli::command());
    assert!(examples.is_empty(), "{}", examples.join("\n"));
    let mut paths: Vec<&str> = META.iter().map(|m| m.path).collect();
    paths.sort_unstable();
    let before = paths.len();
    paths.dedup();
    assert_eq!(before, paths.len(), "a leaf has two metadata rows");
}

/// T2: a leaf has exactly one row in each table the spec is built from, and
/// every row names a leaf: metadata, spec facts, error codes, the report
/// schemas of its outputs. (The examples: `check_examples`, above.)
#[test]
fn every_leaf_has_its_rows_in_every_table_and_no_row_is_an_orphan() {
    let root = cli::command();
    let leaves = schema::leaf_paths(&root);
    assert!(leaves.len() > 70, "the tree has its leaves ({})", leaves.len());
    let facts = canon_cli::spec_facts::check_facts(&root);
    assert!(facts.is_empty(), "{}", facts.join("\n"));
    let mut problems = Vec::new();
    let report_rows = canon_cli::reports::report_schemas();
    for leaf in &leaves {
        if schema::meta_for(leaf).is_none() {
            problems.push(format!("`{leaf}` has no META row"));
        }
        if schema::error_codes(leaf).is_empty() {
            problems.push(format!("`{leaf}` has no error-code row (schema_codes.rs)"));
        }
        if !report_rows.iter().any(|row| row.command == leaf) {
            problems.push(format!("`{leaf}` has no report schema row (reports::report_schemas)"));
        }
    }
    for row in META {
        if !leaves.iter().any(|l| l == row.path) {
            problems.push(format!("META row `{}` names no leaf", row.path));
        }
    }
    for row in &report_rows {
        if !leaves.iter().any(|l| l == row.command) {
            problems.push(format!("report schema row {} names `{}`, which is no leaf", row.name, row.command));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Splits an example invocation into words, honoring single and double quotes.
fn words(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quote, mut started) = (Vec::new(), String::new(), None, false);
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if started || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `meta()` reaches, for every leaf, the row of that leaf: parsing the leaf's
/// example gives a command whose metadata row carries the leaf's own path.
#[test]
fn meta_reaches_the_row_of_every_leaf() {
    let leaves = schema::leaf_paths(&cli::command());
    assert!(!leaves.is_empty());
    for leaf in &leaves {
        let example = schema::example_for(leaf).unwrap_or_else(|| panic!("`{leaf}` has no example"));
        let cli = Cli::try_parse_from(words(example)).unwrap_or_else(|e| panic!("`{example}` does not parse: {e}"));
        assert_eq!(meta(&cli.command).path, leaf, "`{example}` reached the wrong row");
    }
}

/// Bare `canon brief` ran in 0.27; it is now a usage error naming `brief show`.
#[test]
fn bare_brief_is_a_usage_error_naming_brief_show() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(["brief", "--json"]).output().expect("spawn canon");
    assert_eq!(out.status.code(), Some(2));
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).expect("a JSON error");
    assert_eq!(body["error"]["code"], "USAGE");
    assert_eq!(body["error"]["expected"], serde_json::json!(["brief show"]));
}
