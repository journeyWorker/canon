//! T3 (0.28 D1): the options of every leaf obey one grammar.
//!
//! - every option says what it is (help) and, when it takes a value, names it
//!   from the placeholder list; a closed set lists its `possibleValues`; a
//!   default is a real one and matches the per-leaf table;
//! - one option name keeps one value name, arity and value set on every leaf
//!   (a leaf may declare a subset of a shared set: [`DECLARED_SUBSETS`]);
//! - `docs/design/cli-vocabulary.tsv` and the spec use the same options, both
//!   ways, with the same value names;
//! - no valued option is named like a verb, no leaf like `update|get|delete`;
//! - short aliases are `-h -V -o -q` only; `--json`, `--repo`, `--as` mean the
//!   same thing everywhere;
//! - every `conflictsWith` / `requires` the spec lists is enforced when the
//!   command line is parsed.

mod harness;

use std::collections::{BTreeMap, BTreeSet};

use clap::error::{ContextKind, ErrorKind};
use harness::*;
use serde_json::Value;

/// D1's placeholders and the names the per-leaf table adds (`docs/design/cli-commands-0.28.md`, "Value names");
/// the table's rows also use `STATUS` and `HASH`, the typed forms `JSON|FILE|-` and `FORMAT:PATH`, and the concept ids (`*_ID`).
const PLACEHOLDERS: &[&str] = &[
    "DIR", "FILE", "PATH", "ID", "SHA", "N", "TIMESTAMP", "NAME", "TEXT", "URL", "KIND", "FIELDS", // D1
    "REV", "DIGEST", "LIST", "TAG", "LEVEL", "TIER", "STATE", "SHELL", "KEY", "CURSOR", "CLASS", "CODE", "SEVERITY", "DISPOSITION", "RUN_OUTCOME", "REVIEW_OUTCOME", "RESULT", "VERDICT", "ROLE", "CHANGE_KIND", "FLOW_ID", "PATH_OR_EFFECT", "NAME@VERSION", // the table
    "STATUS", "HASH", "JSON|FILE|-", "FORMAT:PATH", // used by table rows
];

/// A leaf that takes an option's value from a subset of the shared set: (option, leaf).
const DECLARED_SUBSETS: &[(&str, &str)] = &[("kind", "purge"), ("kind", "gate promote")];

/// Options that name an act, not a mode; a mode is a flag, never a valued option.
const VERBS: &[&str] = &["add", "new", "list", "show", "check", "verify", "validate", "approve", "accept", "move", "update", "get", "delete", "remove", "create", "set"];

/// Leaves with no `--repo`: they need no repo (`docs/design/cli-commands-0.28.md`, Shared).
const NO_REPO: &[&str] = &["schema", "selftest", "completion", "regime show", "skills show"];

/// Short aliases the grammar allows; `-h` and `-V` are clap's own, `-o` is `--output`.
const SHORTS: &[(char, Option<&str>)] = &[('h', None), ('V', None), ('o', Some("output")), ('q', None)];

/// An option that is repeatable on one leaf and single on another, by the per-leaf table's own rows:
/// `finding add --scenario-id <SCENARIO_ID>...` names every scenario a finding is about, `evidence add` takes one.
const REPEATABLE_DIFFERS: &[&str] = &["scenario-id"];

fn all_options() -> Vec<(String, &'static Value)> {
    let mut out = Vec::new();
    for node in leaves() {
        for option in options(node) {
            out.push((path_of(node), option));
        }
    }
    out
}

fn name(option: &Value) -> &str {
    option["name"].as_str().expect("an option has a name")
}

#[test]
fn every_option_says_what_it_is() {
    let mut problems = Vec::new();
    for (leaf, option) in all_options().into_iter().chain(spec()["globalOptions"].as_array().unwrap().iter().map(|o| ("(global)".to_string(), o))) {
        if !option["help"].as_str().is_some_and(|h| !h.trim().is_empty()) {
            problems.push(format!("`{leaf}` --{}: no help", name(option)));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_value_is_named_from_the_placeholder_list_and_a_flag_names_none() {
    let mut problems = Vec::new();
    for (leaf, option) in all_options() {
        match (option["kind"].as_str().unwrap(), option["valueName"].as_str()) {
            ("value", Some(value)) => {
                if !PLACEHOLDERS.contains(&value) && !value.ends_with("_ID") {
                    problems.push(format!("`{leaf}` --{}: value name {value} is not in the placeholder list", name(option)));
                }
            }
            ("value", None) => problems.push(format!("`{leaf}` --{}: a valued option without a value name", name(option))),
            (_, Some(value)) => problems.push(format!("`{leaf}` --{}: a flag names the value {value}", name(option))),
            _ => {}
        }
    }
    for node in leaves() {
        for operand in node["operands"].as_array().unwrap() {
            let kind = operand["kind"].as_str().unwrap();
            // D1 r1 names `feature new <SURFACE>`.
            if !PLACEHOLDERS.contains(&kind) && !kind.ends_with("_ID") && kind != "SURFACE" {
                problems.push(format!("`{}` operand {}: kind {kind} is not in the placeholder list", path_of(node), operand["name"]));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Every `{a,b}` the per-leaf table writes after an option, in a table row (the shared-options
/// table and the leaf rows; the prose paragraphs under them describe). `--as` is a closed set; attribution `--role` is an open
/// slug (owner decision 2026-10-23), so it has no possible values and is never listed here.
fn closed_options() -> BTreeSet<String> {
    let text = std::fs::read_to_string(repo_root().join("docs/design/cli-commands-0.28.md")).unwrap();
    let mut out = BTreeSet::new();
    for line in text.lines().take_while(|l| !l.starts_with("## 0.27")).filter(|l| l.starts_with('|')) {
        for (i, span) in line.split('`').enumerate() {
            if i % 2 == 1 {
                if let Some(option) = span.trim().split_once(", ").map_or(span.trim(), |(_, rest)| rest).strip_prefix("--") {
                    if option.contains('{') {
                        out.insert(option.split(|c: char| !(c.is_ascii_lowercase() || c == '-')).next().unwrap().to_string());
                    }
                }
            }
        }
    }
    out
}

#[test]
fn a_closed_set_lists_its_possible_values_and_a_default_is_a_real_one() {
    let closed = closed_options();
    assert!(closed.len() > 10, "the table names closed sets: {closed:?}");
    let mut problems = Vec::new();
    for (leaf, option) in all_options() {
        let n = name(option);
        let values = strings(&option["possibleValues"]);
        if closed.contains(n) && values.is_empty() {
            problems.push(format!("`{leaf}` --{n}: the table writes a closed set, the spec lists no possibleValues"));
        }
        let default = strings(&option["default"]);
        if default.iter().any(|d| d.is_empty()) {
            problems.push(format!("`{leaf}` --{n}: the default is the empty string, which is no default"));
        }
        if !values.is_empty() && !default.is_empty() && !default.iter().all(|d| values.contains(d)) {
            problems.push(format!("`{leaf}` --{n}: default {default:?} is outside {values:?}"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The per-leaf table is normative: its defaults, closed sets and arities are the spec's.
#[test]
fn the_spec_carries_the_defaults_sets_and_arities_the_table_writes() {
    let mut problems = Vec::new();
    let mut compared = 0;
    for (leaf_path, rows) in table() {
        let Some(node) = leaves().into_iter().find(|n| path_of(n) == *leaf_path) else {
            problems.push(format!("`{leaf_path}` is a row of the table and no leaf of the spec"));
            continue;
        };
        for row in rows {
            // `--as` is declared once, globally.
            if row.name == "as" {
                continue;
            }
            let Some(option) = option(node, &row.name) else {
                problems.push(format!("`{leaf_path}` --{}: in the table, not in the spec", row.name));
                continue;
            };
            compared += 1;
            let default = strings(&option["default"]);
            match &row.default {
                Some(d) if default != [d.clone()] => problems.push(format!("`{leaf_path}` --{}: the table's default is {d:?}, the spec's {default:?}", row.name)),
                None if !default.is_empty() => problems.push(format!("`{leaf_path}` --{}: the spec's default {default:?} is not in the table", row.name)),
                _ => {}
            }
            if !row.set.is_empty() {
                let set: BTreeSet<String> = row.set.iter().cloned().collect();
                let listed: BTreeSet<String> = strings(&option["possibleValues"]).into_iter().collect();
                if set != listed {
                    problems.push(format!("`{leaf_path}` --{}: the table's set is {set:?}, the spec's {listed:?}", row.name));
                }
            }
            if let Some(value) = &row.value {
                if option["valueName"].as_str() != Some(value.as_str()) {
                    problems.push(format!("`{leaf_path}` --{}: the table's value name is {value}, the spec's {}", row.name, option["valueName"]));
                }
            }
            // The table writes `...` where an option repeats; a row without it does not claim the option is single.
            if row.repeatable && option["repeatable"] != true {
                problems.push(format!("`{leaf_path}` --{}: the table repeats it, the spec's option is single", row.name));
            }
        }
    }
    assert!(compared > 150, "the table's options were compared: {compared}");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn one_name_has_one_value_name_arity_and_value_set_on_every_leaf() {
    let mut by_name: BTreeMap<String, Vec<(String, &Value)>> = BTreeMap::new();
    for (leaf, option) in all_options() {
        by_name.entry(name(option).to_string()).or_default().push((leaf, option));
    }
    let mut problems = Vec::new();
    for (n, uses) in &by_name {
        let shape = |option: &Value| (option["kind"].clone(), option["valueName"].clone(), if REPEATABLE_DIFFERS.contains(&n.as_str()) { Value::Null } else { option["repeatable"].clone() });
        let first = shape(uses[0].1);
        for (leaf, option) in uses {
            if shape(option) != first {
                problems.push(format!("--{n}: `{}` is {first:?} but `{leaf}` is {:?}", uses[0].0, shape(option)));
            }
        }
        // Value sets: equal, except a declared subset of the shared (largest) set.
        let largest: BTreeSet<String> = uses.iter().map(|(_, o)| strings(&o["possibleValues"]).into_iter().collect::<BTreeSet<_>>()).max_by_key(BTreeSet::len).unwrap();
        for (leaf, option) in uses {
            let set: BTreeSet<String> = strings(&option["possibleValues"]).into_iter().collect();
            let declared = DECLARED_SUBSETS.iter().any(|(o, l)| o == n && l == leaf);
            if declared {
                if !(set.is_subset(&largest) && set.len() < largest.len()) {
                    problems.push(format!("--{n} on `{leaf}` is a declared subset but is not a strict subset of {largest:?}: {set:?}"));
                }
            } else if set != largest {
                problems.push(format!("--{n}: `{leaf}` has the value set {set:?}, the shared set is {largest:?} (declare a subset in DECLARED_SUBSETS only with a reason in the plan)"));
            }
        }
    }
    for (option, leaf) in DECLARED_SUBSETS {
        assert!(by_name.get(*option).is_some_and(|uses| uses.iter().any(|(l, _)| l == leaf)), "DECLARED_SUBSETS names --{option} on `{leaf}`, which does not exist");
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_vocabulary_and_the_spec_use_the_same_options_with_the_same_value_names() {
    let vocabulary = vocabulary();
    let mut seen = BTreeSet::new();
    for row in vocabulary {
        assert!(seen.insert(row.option.as_str()), "--{} is in the vocabulary twice", row.option);
        assert!(!row.concept.trim().is_empty(), "--{} has no concept", row.option);
        if let Some(value) = &row.value_name {
            assert!(PLACEHOLDERS.contains(&value.as_str()) || value.ends_with("_ID"), "--{}: {value} is not in the placeholder list", row.option);
        }
    }
    let mut used: BTreeMap<String, (Option<String>, String)> = BTreeMap::new();
    for (leaf, option) in all_options() {
        used.entry(name(option).to_string()).or_insert((option["valueName"].as_str().map(str::to_string), leaf));
    }
    for option in spec()["globalOptions"].as_array().unwrap() {
        used.entry(name(option).to_string()).or_insert((option["valueName"].as_str().map(str::to_string), "(global)".into()));
    }
    let mut problems = Vec::new();
    for (n, (value, leaf)) in &used {
        match vocabulary.iter().find(|r| &r.option == n) {
            None => problems.push(format!("--{n} (`{leaf}`) is not in docs/design/cli-vocabulary.tsv")),
            Some(row) if row.value_name != *value => problems.push(format!("--{n}: the vocabulary says {:?}, `{leaf}` takes {value:?}", row.value_name)),
            Some(_) => {}
        }
    }
    for row in vocabulary {
        if !used.contains_key(&row.option) {
            problems.push(format!("--{} is in the vocabulary and on no leaf", row.option));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn no_valued_option_is_a_verb_and_no_leaf_is_update_get_or_delete() {
    let mut problems = Vec::new();
    for (leaf, option) in all_options() {
        if option["kind"] == "value" && VERBS.contains(&name(option)) {
            problems.push(format!("`{leaf}` --{}: a valued option named like a verb (a mode is a flag)", name(option)));
        }
    }
    for node in leaves() {
        let path = path_of(node);
        let last = path.rsplit(' ').next().unwrap();
        if ["update", "get", "delete"].contains(&last) {
            problems.push(format!("`{path}`: D1 does not use `{last}`"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn short_aliases_are_h_v_o_and_q_only() {
    fn walk(cmd: &clap::Command, path: &str, problems: &mut Vec<String>, shorts: &mut usize) {
        for arg in cmd.get_arguments() {
            let Some(short) = arg.get_short() else { continue };
            *shorts += 1;
            match SHORTS.iter().find(|(c, _)| *c == short) {
                None => problems.push(format!("`{path}` -{short} (--{}): not an allowed short alias", arg.get_long().unwrap_or_default())),
                Some((_, Some(long))) if arg.get_long() != Some(*long) => problems.push(format!("`{path}` -{short} is --{}, the grammar gives it to --{long}", arg.get_long().unwrap_or_default())),
                _ => {}
            }
        }
        for sub in cmd.get_subcommands() {
            walk(sub, &format!("{path} {}", sub.get_name()), problems, shorts);
        }
    }
    let mut root = canon_cli::cli::command();
    root.build();
    let (mut problems, mut shorts) = (Vec::new(), 0);
    walk(&root, "canon", &mut problems, &mut shorts);
    assert!(shorts > 2, "the walk found the short aliases ({shorts})");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn json_repo_and_as_mean_the_same_everywhere() {
    let mut problems = Vec::new();
    let global_as = spec()["globalOptions"].as_array().unwrap().iter().find(|o| o["name"] == "as").expect("--as is global");
    assert_eq!(strings(&global_as["possibleValues"]), ["planner", "implementer", "reviewer", "orchestrator"]);
    assert_eq!(global_as["valueName"], "ROLE");
    for node in leaves() {
        let path = path_of(node);
        match option(node, "json") {
            Some(json) if json["kind"] == "flag" && json["short"].is_null() && strings(&json["aliases"]).is_empty() => {}
            other => problems.push(format!("`{path}`: --json is {other:?}, the output-only flag")),
        }
        match (option(node, "repo"), NO_REPO.contains(&path.as_str())) {
            (Some(repo), false) => {
                if repo["valueName"] != "DIR" || repo["repeatable"] != false || repo["required"] != false {
                    problems.push(format!("`{path}`: --repo is not the optional single <DIR>: {repo}"));
                }
            }
            (None, false) => problems.push(format!("`{path}`: no --repo (only {NO_REPO:?} take none)")),
            (Some(_), true) => problems.push(format!("`{path}`: takes --repo, but needs no repo")),
            (None, true) => {}
        }
        if option(node, "as").is_some() {
            problems.push(format!("`{path}`: redefines --as, which is declared once globally"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// ---------------------------------------------------------------------------
// conflictsWith / requires, enforced at parse time.

/// A value that parses, by the value name the grammar gives it.
fn sample(arg: &clap::Arg) -> String {
    if let Some(first) = arg.get_possible_values().first() {
        return first.get_name().to_string();
    }
    match arg.get_value_names().and_then(|v| v.first()).map(|v| v.as_str()) {
        Some("N") => "1",
        Some("TIMESTAMP") => "2030-01-01",
        Some("FORMAT:PATH") => "junit:x.xml",
        Some("SCENARIO_ID") => "auth.login.01",
        Some("CHANGE_ID") => "add-login",
        Some("PROJECT_ID") => "root",
        Some("SUBJECT_ID") => "demo",
        Some("TASK_ID") => "add-login#1",
        Some("RUN_ID") | Some("STRATEGY_ID") | Some("PARENT_RUN_ID") => "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        Some("NAME@VERSION") => "reviewer@v1",
        Some("SHA") => "0123456789abcdef0123456789abcdef01234567",
        _ => "x",
    }
    .into()
}

fn node<'a>(root: &'a clap::Command, path: &str) -> &'a clap::Command {
    path.split(' ').fold(root, |cmd, word| cmd.find_subcommand(word).unwrap_or_else(|| panic!("no `{word}` in `{path}`")))
}

/// The argument words that give `arg` a value.
fn given(arg: &clap::Arg) -> Vec<String> {
    match arg.get_long() {
        Some(long) if arg.get_action().takes_values() => vec![format!("--{long}"), sample(arg)],
        Some(long) => vec![format!("--{long}")],
        None => vec![sample(arg)],
    }
}

/// The command line of a leaf with everything required given, minus `skip`.
fn required_line(cmd: &clap::Command, path: &str, skip: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = path.split(' ').map(str::to_string).collect();
    for arg in cmd.get_arguments().filter(|a| a.is_required_set() && !skip.contains(&a.get_id().as_str())) {
        line.extend(given(arg));
    }
    line
}

#[test]
fn every_conflict_and_requirement_the_spec_lists_is_enforced_when_parsing() {
    let root = canon_cli::cli::command();
    let (mut conflicts, mut requires) = (0, 0);
    let mut problems = Vec::new();
    for leaf in leaves() {
        let path = path_of(leaf);
        let cmd = node(&root, &path);
        // An option by its long name, an operand by its id (the spec names it that way).
        let arg_of = |option: &str| {
            let bare = option.trim_start_matches("--");
            cmd.get_arguments().find(|a| a.get_long() == Some(bare) || (a.get_long().is_none() && a.get_id() == bare)).unwrap_or_else(|| panic!("`{path}` has no {option}"))
        };
        for option in options(leaf) {
            let own = arg_of(&format!("--{}", name(option)));
            for other in strings(&option["conflictsWith"]) {
                conflicts += 1;
                let other_arg = arg_of(&other);
                let mut line = required_line(cmd, &path, &[own.get_id().as_str(), other_arg.get_id().as_str()]);
                line.extend(given(own));
                line.extend(given(other_arg));
                match root.clone().try_get_matches_from(std::iter::once("canon".to_string()).chain(line.iter().cloned())) {
                    Err(e) if e.kind() == ErrorKind::ArgumentConflict => {}
                    Err(e) => problems.push(format!("`{path}`: --{} with {other} fails, but not as a conflict ({:?}): {}", name(option), e.kind(), e.to_string().lines().next().unwrap_or_default())),
                    Ok(_) => problems.push(format!("`{path}`: --{} and {other} are declared to conflict and parse together: {line:?}", name(option))),
                }
            }
            let needed = strings(&option["requires"]);
            if !needed.is_empty() {
                requires += 1;
                let skip: Vec<&str> = std::iter::once(own.get_id().as_str()).chain(needed.iter().map(|n| arg_of(n).get_id().as_str())).collect();
                let mut line = required_line(cmd, &path, &skip);
                line.extend(given(own));
                match root.clone().try_get_matches_from(std::iter::once("canon".to_string()).chain(line.iter().cloned())) {
                    Err(e) if e.kind() == ErrorKind::MissingRequiredArgument => {
                        let missing = e.get(ContextKind::InvalidArg).map(|v| v.to_string()).unwrap_or_default();
                        if !needed.iter().any(|n| missing.contains(n.as_str())) {
                            problems.push(format!("`{path}`: --{} without {needed:?} is refused for another reason: {missing}", name(option)));
                        }
                    }
                    Err(e) => problems.push(format!("`{path}`: --{} without {needed:?} fails, but not as a missing argument ({:?}): {}", name(option), e.kind(), e.to_string().lines().next().unwrap_or_default())),
                    Ok(_) => problems.push(format!("`{path}`: --{} requires {needed:?} and parses without: {line:?}", name(option))),
                }
            }
        }
    }
    assert!(conflicts > 20 && requires > 5, "the spec lists conflicts ({conflicts}) and requirements ({requires})");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Attribution `--role` is an open slug (owner decision 2026-10-23): policies may require custom
/// roles, so it lists no possible values; the declared role `--as` is the closed set.
#[test]
fn attribution_role_is_open_and_the_declared_role_is_closed() {
    let mut roles = 0;
    for (leaf, option) in all_options() {
        match name(option) {
            "role" => {
                roles += 1;
                assert!(option["possibleValues"].as_array().is_none_or(|v| v.is_empty()), "`{leaf}` --role is an open slug: {option}");
            }
            "as" => assert_eq!(strings(&option["possibleValues"]), ["planner", "implementer", "reviewer", "orchestrator"], "`{leaf}` --as"),
            _ => {}
        }
    }
    assert!(roles >= 8, "the attribution leaves take --role ({roles})");
}
