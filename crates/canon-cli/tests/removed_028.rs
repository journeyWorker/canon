//! The removed-spellings table (0.27 → 0.28 and the 0.31 id options), run row by row against the built `canon`
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
            let hint = body["error"]["hint"].as_str().unwrap_or_default();
            let expected: Vec<&str> = body["error"]["expected"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            // A group that once ran bare is the contract's COMMAND_REQUIRED, with the replacement in its hint.
            if row.kind == Kind::Bare {
                assert_eq!(body["error"]["code"], "COMMAND_REQUIRED", "canon {shown}: {body}");
                assert!(hint.contains(row.new), "canon {shown}: hint lacks {:?}: {hint}", row.new);
                exercised += 1;
                continue;
            }
            assert_eq!(body["error"]["code"], "USAGE", "canon {shown}: {body}");
            if row.operand {
                // The typed leaf when it is one the row applies to, else the row's replacement.
                let typed = argv[..argv.len() - 2].join(" ");
                let leaf = if row.scope.contains(&row.new) { typed.as_str() } else { row.new };
                assert!(hint.contains(&format!("canon {leaf}")), "canon {shown}: hint lacks `canon {leaf}`: {hint}");
                assert!(expected.iter().any(|line| line.starts_with(leaf)), "canon {shown}: expected lacks {leaf:?}: {expected:?}");
            } else {
                assert!(hint.contains(row.new), "canon {shown}: hint lacks {:?}: {hint}", row.new);
                assert!(expected.contains(&row.new), "canon {shown}: expected lacks {:?}: {expected:?}", row.new);
            }
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

/// A removed option whose value became an operand: the hint is the line the
/// caller typed, on the leaf it typed, with the value filled in, and that line
/// parses against the CLI.
#[test]
fn an_operand_hint_names_the_typed_leaf_with_the_typed_values() {
    let cases: &[(&str, &str)] = &[
        ("brief show --change-id pause-menu", "brief show pause-menu"),
        ("brief check --change-id pause-menu", "brief check pause-menu"),
        ("brief check --change=pause-menu", "brief check pause-menu"),
        ("brief approve --change-id pause-menu --approver alice --note ok", "brief approve pause-menu --approver alice --note ok"),
        ("brief approve --change pause-menu --approver alice --note ok", "brief approve pause-menu --approver alice --note ok"),
        ("subject adopt --subject-id cart --change-id add-cart", "subject adopt cart --change add-cart"),
        ("subject unadopt --subject-id cart --change-id add-cart", "subject unadopt cart --change add-cart"),
        ("dispatch end --run-id 01ARZ3NDEKTSV4RRFFQ69G5FAV --run-outcome succeeded", "dispatch end 01ARZ3NDEKTSV4RRFFQ69G5FAV --run-outcome succeeded"),
        ("prompt show --name p --version 1", "prompt show p@1"),
        ("prompt show --version 1 --name p", "prompt show p@1"),
        ("prompt add --name p --version 1 --manifest m.json", "prompt add p@1 --manifest m.json"),
        ("divergence add --scenario-id a.b.01 --actor-id x", "divergence add a.b.01 --actor x"),
        ("divergence defer --scenario-id a.b.01 --actor-id x --reason r --expiry 2030-01-01", "divergence defer a.b.01 --actor x --reason r --expiry 2030-01-01"),
        ("divergence resolve --scenario-id a.b.01 --actor-id x", "divergence resolve a.b.01 --actor x"),
        ("divergence accept --scenario-id a.b.01 --reason r --tracking t --actor-id x", "divergence accept a.b.01 --reason r --tracking t --actor x"),
        ("finding close --change-id c1 --round 1 --seq 1 --disposition fixed", "finding close c1 --round 1 --seq 1 --disposition fixed"),
        ("review scenario --scenario-id a.b.01 --actor-id x --upstream-ref u", "review scenario a.b.01 --actor x --upstream-ref u"),
        ("review change --change-id c1 --actor-id x --upstream-ref u", "review change c1 --actor x --upstream-ref u"),
        ("review scenario --change-id c1 --actor-id x --upstream-ref u", "review change c1 --actor x --upstream-ref u"),
        ("review change --scenario-id a.b.01 --actor-id x --upstream-ref u", "review scenario a.b.01 --actor x --upstream-ref u"),
        ("report --snapshot out", "report snapshot out"),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (typed, runnable) in cases {
        let argv: Vec<&str> = typed.split(' ').collect();
        let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(&argv).arg("--json").current_dir(dir.path()).output().expect("spawn canon");
        assert_eq!(out.status.code(), Some(2), "canon {typed}: {}", String::from_utf8_lossy(&out.stdout));
        let body: Value = serde_json::from_slice(&out.stdout).unwrap();
        let expected: Vec<&str> = body["error"]["expected"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        let first = expected.first().copied().unwrap_or_default();
        // `--json` was typed after the leaf, so it rides along; the runnable line is the typed line less the removed spelling.
        assert_eq!(first, format!("{runnable} --json"), "canon {typed}: {body}");
        let hint = body["error"]["hint"].as_str().unwrap();
        assert!(hint.contains(&format!("canon {first}")), "canon {typed}: {hint}");
        let words: Vec<&str> = std::iter::once("canon").chain(first.split(' ')).collect();
        if let Err(e) = tree().try_get_matches_from(words) {
            panic!("`canon {first}` does not parse: {e}");
        }
    }
}

/// The 0.30 id options became bare nouns on exactly the 67 places of 24 commands the table lists: every row's
/// leaf has the new option and not the old one, and every row is of release 0.31.0.
#[test]
fn the_bare_noun_rows_cover_the_67_places_of_the_14_options() {
    let root = tree();
    let rows: Vec<&Row> = ROWS.iter().filter(|row| row.since == canon_cli::removed_028::V31).collect();
    assert_eq!(rows.len(), 14, "one row per option");
    let mut commands = std::collections::BTreeSet::new();
    let mut places = 0;
    for row in &rows {
        assert_eq!(row.kind, Kind::Option, "{row:?}");
        assert_eq!(row.new, format!("--{}", row.old.trim_start_matches("--").strip_suffix("-id").expect("every old spelling ends in -id")), "{row:?}");
        for scope in row.scope {
            let node = resolve(&root, scope).unwrap_or_else(|| panic!("`{scope}` does not exist: {row:?}"));
            assert!(defines_option(node, row.new), "`{scope}` has no {}", row.new);
            assert!(!defines_option(node, row.old), "`{scope}` still has {}", row.old);
            commands.insert(*scope);
            places += 1;
        }
    }
    assert_eq!((places, commands.len()), (67, 24));
}

/// No option of any command ends in `-id`, and a removed row names an old spelling only on commands that no longer parse it.
#[test]
fn no_option_of_the_tree_ends_in_id() {
    fn walk(node: &clap::Command, path: &str, found: &mut Vec<String>) {
        for arg in node.get_arguments() {
            if let Some(long) = arg.get_long().filter(|long| long.ends_with("-id")) {
                found.push(format!("{path}: --{long}"));
            }
        }
        for sub in node.get_subcommands() {
            walk(sub, &format!("{path} {}", sub.get_name()), found);
        }
    }
    let mut found = Vec::new();
    walk(&tree(), "canon", &mut found);
    assert!(found.is_empty(), "{found:#?}");
}

/// A line with several removed spellings is refused once, and the hint line is the typed line with all of them
/// replaced: it parses against the tree, one 0.28 operand move included.
#[test]
fn the_hint_of_a_line_with_several_removed_spellings_parses() {
    let cases: &[(&str, &str)] = &[
        ("evidence add --scenario-id a.b.01 --project-id root --actor-id x --session-id s --class test-run --ref r", "--scenario-id"),
        ("finding add --change-id c --round 1 --seq 1 --severity note --actor-id x --scenario-id a.b.01 --scenario-id a.b.02 --review-id 01ARZ3NDEKTSV4RRFFQ69G5FAV --summary s --introduced-unknown", "--change-id"),
        ("scenario new a.b.01 --title T --subject-id s --change-id=c --flow F1 --actor-id x", "--subject-id"),
        ("review scenario --scenario-id a.b.01 --project-id root --actor-id x --session-id s --upstream-ref u", "--scenario-id"),
        ("subject adopt --subject-id cart --change-id add-cart", "--subject-id"),
        ("evidence audit a.b.01 --ref r --evaluator-id e --evaluator-version 1 --evaluator-digest 0000000000000000000000000000000000000000000000000000000000000000 --audit-result confirmed --run-id 01ARZ3NDEKTSV4RRFFQ69G5FAV", "--evaluator-id"),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (typed, first) in cases {
        let argv: Vec<&str> = typed.split(' ').collect();
        let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(&argv).arg("--json").current_dir(dir.path()).output().expect("spawn canon");
        assert_eq!(out.status.code(), Some(2), "canon {typed}: {}", String::from_utf8_lossy(&out.stdout));
        let body: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(body["error"]["field"], *first, "{body}");
        let hint = body["error"]["hint"].as_str().unwrap();
        let line = hint.split('`').nth(1).unwrap_or_else(|| panic!("no command line in {hint}"));
        assert!(!line.split(' ').any(|w| w.starts_with("--") && w.split('=').next().is_some_and(|name| name.ends_with("-id"))), "an old spelling is left in {line}");
        let words: Vec<&str> = line.split(' ').collect();
        if let Err(e) = tree().try_get_matches_from(words) {
            panic!("`{line}` does not parse: {e}");
        }
    }
}
