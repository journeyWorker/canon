//! The "Removed in 0.28" table, run row by row against the built `canon`
//! binary: every old spelling the clap tree no longer parses exits 2 with an
//! `error.hint` that names the replacement, and lists it in `error.expected`.

use std::process::Command;

use canon_cli::removed_028::{Kind, Row, ROWS};
use serde_json::Value;

fn tree() -> clap::Command {
    let mut root = canon_cli::cli::command();
    root.build();
    root
}

fn resolve<'a>(root: &'a clap::Command, path: &str) -> Option<&'a clap::Command> {
    path.split(' ').filter(|word| !word.is_empty()).try_fold(root, |node, word| node.find_subcommand(word))
}

fn defines_option(node: &clap::Command, old: &str) -> bool {
    match old.strip_prefix("--") {
        Some(long) => node.get_arguments().any(|arg| arg.get_long() == Some(long)),
        None => old.chars().nth(1).is_some_and(|short| node.get_arguments().any(|arg| arg.get_short() == Some(short))),
    }
}

/// The argv that types the removed spelling of `row`, once per place it applies,
/// for those the tree no longer parses.
fn invocations(root: &clap::Command, row: &Row) -> Vec<Vec<String>> {
    let words = |path: &str| path.split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect::<Vec<_>>();
    match row.kind {
        Kind::Command => {
            if resolve(root, row.old).is_some() {
                return Vec::new();
            }
            vec![words(row.old)]
        }
        Kind::Bare => match resolve(root, row.old) {
            Some(group) if group.has_subcommands() => vec![words(row.old)],
            _ => Vec::new(),
        },
        Kind::Option => row
            .scope
            .iter()
            .filter_map(|scope| {
                let node = resolve(root, scope)?;
                if defines_option(node, row.old) {
                    return None;
                }
                let mut argv = words(scope);
                argv.push(row.old.to_string());
                argv.push("value".to_string());
                Some(argv)
            })
            .collect(),
    }
}

#[test]
fn every_removed_spelling_exits_2_with_a_hint_naming_its_replacement() {
    let root = tree();
    let dir = tempfile::tempdir().unwrap();
    let mut exercised = 0;
    for row in ROWS {
        for argv in invocations(&root, row) {
            let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(&argv).arg("--json").current_dir(dir.path()).output().expect("spawn canon");
            let shown = argv.join(" ");
            assert_eq!(out.status.code(), Some(2), "canon {shown}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            assert!(out.stderr.is_empty(), "canon {shown}: JSON mode writes nothing on stderr");
            let body: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("canon {shown}: stdout is not JSON ({e})"));
            assert_eq!(body["ok"], Value::Bool(false), "canon {shown}: {body}");
            assert_eq!(body["error"]["code"], "USAGE", "canon {shown}: {body}");
            let hint = body["error"]["hint"].as_str().unwrap_or_default();
            assert!(hint.contains(row.new), "canon {shown}: hint lacks {:?}: {hint}", row.new);
            let expected: Vec<&str> = body["error"]["expected"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            assert!(expected.contains(&row.new), "canon {shown}: expected lacks {:?}: {expected:?}", row.new);
            exercised += 1;
        }
    }
    // Rows another slice has not landed yet (the old spelling still parses) are skipped; the rest is most of the table.
    assert!(exercised * 2 > ROWS.len(), "only {exercised} of {} rows were exercised", ROWS.len());
}

/// A replacement the table names is a spelling the tree has: a command path
/// resolves, an option is defined on the leaf the row's scope names.
#[test]
fn every_replacement_the_table_names_exists() {
    let root = tree();
    for row in ROWS {
        let new = row.new;
        if new.starts_with("--") {
            let options: Vec<&str> = new.split(' ').filter(|w| w.starts_with("--")).collect();
            for scope in row.scope {
                let Some(node) = resolve(&root, scope) else { continue };
                for option in &options {
                    // A replacement option is on the same leaf, or (`--input`, `--commit`) shared by write leaves.
                    if !defines_option(node, option) {
                        assert!(["--input", "--commit", "--plan-digest", "--rev", "--resolution-rev", "--introduced-rev", "--plan-source", "--output"].contains(option), "`{scope}` has no {option}: {row:?}");
                    }
                }
            }
        } else {
            let path: Vec<&str> = new.split(' ').take_while(|w| !w.starts_with('-')).collect();
            let found = resolve(&root, &path.join(" ")).is_some();
            // Rows for leaves another slice lands (`artifacts move`, `artifacts store`, `ingest records`) resolve after the merge.
            let landing_elsewhere = ["artifacts move", "artifacts store", "ingest records"].iter().any(|p| path.join(" ") == *p);
            assert!(found || landing_elsewhere, "`canon {}` does not exist: {row:?}", path.join(" "));
        }
    }
}

/// An old spelling a clap parse refuses but the table does not know stays an
/// ordinary usage error: no invented hint.
#[test]
fn an_unknown_spelling_is_an_ordinary_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["gate", "check", "--no-such-option", "--json"]).current_dir(dir.path()).output().expect("spawn canon");
    assert_eq!(out.status.code(), Some(2));
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(!body["error"]["hint"].as_str().unwrap_or_default().contains("was removed in"), "{body}");
}

/// The spec's `removed` section is the table, row for row.
#[test]
fn the_spec_carries_the_whole_table() {
    let spec = canon_cli::spec::generate();
    let removed = spec["removed"].as_array().expect("removed section");
    assert_eq!(removed.len(), ROWS.len());
    for (spec_row, row) in removed.iter().zip(ROWS) {
        assert_eq!(spec_row["old"], row.old);
        assert_eq!(spec_row["new"], row.new);
    }
}
