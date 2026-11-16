//! `canon-0-42-change-drop`, F8: the guides, the help, the generated spec and the CHANGELOG say how a human closes a change, that six leaves are
//! human-only, and how a closed change looks; the checks that find a guide line that does not fit the schema, a human-only list that is not six and a
//! CHANGELOG without the caller note. Each test names the scenario it proves.

#[path = "support/hermetic.rs"]
mod hermetic;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// The leaves a human decides (`x-canon.roles` is empty): `change.drop.24` says six, the scenario `skills.install.21` lists them.
const LEAVES: [&str; 6] = ["brief approve", "policy approve", "divergence accept", "change drop", "change defer", "change reopen"];
/// The guides that carry the closing procedure: the core guide (`SKILL.src.md`), the planner, the orchestrator and the subject reference.
const GUIDES: [&str; 4] = ["canon", "role-planner", "role-orchestrator", "canon-subject"];
/// The options of the closing line: the human's words, where the decision lives, who decided, the relay and the agent.
const CLOSING_OPTIONS: [&str; 5] = ["reason", "ref", "approver", "relayed-via", "actor"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `text` with every run of white space read as one space: a guide wraps its lines where it likes.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn canon(repo: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(repo).env_remove("CANON_ROLE").output().expect("spawn canon binary")
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn guide(name: &str) -> &'static str {
    canon_cli::guide::guide(name).unwrap_or_else(|| panic!("the binary embeds no guide `{name}`"))
}

fn changelog() -> String {
    std::fs::read_to_string(repo_root().join("CHANGELOG.md")).unwrap()
}

/// The closing command line a guide gives: from `canon change defer <` to the next backtick, in the guide's flat text (a code span or a fenced block).
fn closing_line(said: &str) -> Option<String> {
    let start = said.find("canon change defer <")?;
    let end = start + said[start..].find('`')?;
    Some(said[start..end].trim().to_string())
}

/// What is wrong with a closing command line against the CLI's own tree: the leaf does not exist, an option is not one of the leaf's, or an option of
/// the procedure is missing.
fn line_problems(root: &clap::Command, name: &str, line: &str) -> Vec<String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let leaf = match (words.first(), words.get(1), words.get(2)) {
        (Some(&"canon"), Some(&"change"), Some(leaf)) => root.find_subcommand("change").and_then(|change| change.find_subcommand(leaf)),
        _ => None,
    };
    let Some(leaf) = leaf else { return vec![format!("the guide `{name}` line `{line}` names no leaf of the CLI")] };
    let mut problems = Vec::new();
    for word in &words {
        if let Some(option) = word.strip_prefix("--") {
            if !leaf.get_arguments().any(|arg| arg.get_long() == Some(option)) {
                problems.push(format!("the guide `{name}` line `{line}`: `--{option}` is not an option of `canon change {}`", leaf.get_name()));
            }
        }
    }
    for option in CLOSING_OPTIONS {
        if !words.contains(&format!("--{option}").as_str()) {
            problems.push(format!("the guide `{name}` line `{line}` never gives `--{option}`"));
        }
    }
    problems
}

/// What a guide lacks of the closing story: the six human-only leaves, the relay, the closing line (which fits the CLI), defer and drop, the new
/// approval after a reopen, a shipped change, and how a closed change looks in `status`, `gate check` and `query`.
fn guide_problems(root: &clap::Command, name: &str, body: &str) -> Vec<String> {
    let said = flat(body);
    let lower = said.to_lowercase();
    let mut problems = Vec::new();
    for leaf in LEAVES {
        if !said.contains(leaf) {
            problems.push(format!("the guide `{name}` never names the human-only leaf `{leaf}`"));
        }
    }
    if !(lower.contains("six leaves") || lower.contains("are six")) {
        problems.push(format!("the guide `{name}` never says the human-only leaves are six"));
    }
    for needle in ["--relayed-via", "`setAsideChanges`", "`setAsideScenarios`", "`closed_change`", "change_state", "not this release", "new `brief approve`"] {
        if !said.contains(needle) {
            problems.push(format!("the guide `{name}` never says `{needle}`"));
        }
    }
    if !lower.contains("shipped change cannot be closed") {
        problems.push(format!("the guide `{name}` never says a shipped change cannot be closed"));
    }
    match closing_line(&said) {
        None => problems.push(format!("the guide `{name}` has no closing line `canon change defer <change> --reason ...`")),
        Some(line) => problems.extend(line_problems(root, name, &line)),
    }
    problems
}

/// The human-only leaves of `closed` (the leaves whose `x-canon.roles` is empty), against the six.
fn human_only_problems(closed: &[String]) -> Vec<String> {
    let mut got: Vec<&str> = closed.iter().map(String::as_str).collect();
    got.sort_unstable();
    let mut want = LEAVES.to_vec();
    want.sort_unstable();
    if got == want {
        Vec::new()
    } else {
        vec![format!("the human-only leaves are {got:?}, not the six {want:?}")]
    }
}

/// What the Unreleased section of `changelog` lacks about this change: the Added entries, the Changed entries and the `**Callers:**` note with its four facts.
fn release_problems(changelog: &str) -> Vec<String> {
    let Some(unreleased) = changelog.split("\n## ").find(|section| section.starts_with("Unreleased")) else { return vec!["the CHANGELOG has no Unreleased section".to_string()] };
    let mut problems = Vec::new();
    let part = |name: &str| unreleased.split("\n### ").find(|part| part.starts_with(name));
    let needs = |label: &str, text: Option<&str>, needles: &[&str], problems: &mut Vec<String>| match text {
        None => problems.push(format!("the Unreleased section has no {label} part")),
        Some(text) => {
            for needle in needles {
                if !text.contains(needle) {
                    problems.push(format!("the {label} part never says `{needle}`"));
                }
            }
        }
    };
    needs(
        "Added",
        part("Added"),
        &[
            "`canon change defer`",
            "`canon change drop`",
            "`canon change reopen`",
            "`change_state`",
            "`CHANGE_CONFLICT`",
            "`CHANGE_RECORDS_KEPT`",
            "`CHANGE_HISTORY_MISSING`",
            "`closed_change`",
            "`setAsideChanges`",
            "`setAsideScenarios`",
            "`statusVersion` stays 4",
            "`canon migrate`",
        ],
        &mut problems,
    );
    needs("Changed", part("Changed"), &["`routing:`", "`change_state: local`", "`APPROVED_BRIEF`", "`CHANGE_ARGUMENT_INVALID`", "thirteen", "writes a new line"], &mut problems);
    let Some(start) = unreleased.find("**Callers:**") else {
        problems.push("the Unreleased section has no `**Callers:**` note".to_string());
        return problems;
    };
    let note = &unreleased[start..];
    let facts: [(&str, &[&str]); 4] = [
        ("a closed change drops out of the requirements", &["closed change drops out of the requirements"]),
        ("a repo without the routing line runs `canon migrate` before the closing commands", &["`change_state` line under `routing:`", "`canon migrate` before the closing commands"]),
        ("canon 0.41 and older refuse the canon.yaml with CONFIG_INVALID and deleting the `change_state:` line by hand goes back", &["0.41 and older", "`CONFIG_INVALID`", "`canon migrate --revert`", "by hand", "required again"]),
        ("a hand-written .gitignore adds `!.canon/ledger/kind=change_state/`", &["`.canon/ledger/*`", "`!.canon/ledger/kind=change_state/`"]),
    ];
    for (fact, needles) in facts {
        if needles.iter().any(|needle| !note.contains(needle)) {
            problems.push(format!("the callers note lacks the fact: {fact}"));
        }
    }
    problems
}

/// change.drop.24 (F8): the guides, the help, the generated spec and `canon init` say how a human closes a change with the relay, that six leaves are
/// human-only, and how a closed change looks in `status`, `gate check` and `query`; the leaves that name no agent role are the six.
#[test]
fn change_drop_24_the_guides_help_and_spec_say_how_to_close_a_change_who_may_and_how_it_looks() {
    let root = canon_cli::cli::command();
    for name in GUIDES {
        let problems = guide_problems(&root, name, guide(name));
        assert!(problems.is_empty(), "{problems:#?}");
    }
    // The guides that list the human-only acts only in passing name all six.
    for name in ["role-implementer", "role-reviewer"] {
        let said = flat(guide(name));
        for leaf in LEAVES {
            assert!(said.contains(leaf), "the guide `{name}` never names `{leaf}`");
        }
    }
    // The planner and the subject reference say where `subject unadopt` sends an approved change.
    for name in ["role-planner", "canon-subject"] {
        let said = flat(guide(name));
        assert!(said.contains("`APPROVED_BRIEF`") && (said.contains("`change defer`") || said.contains("`canon change defer`")), "the guide `{name}` points `APPROVED_BRIEF` at defer and drop");
        assert!(said.contains("change drop"), "the guide `{name}` never names `change drop`");
    }
    // The installed stub names the six, too.
    let stub = canon_cli::guide::stub("canon/SKILL.md", "0.0.0");
    for leaf in LEAVES {
        assert!(stub.contains(leaf), "the stub never names `{leaf}`");
    }

    // The command listing: the leaves that name no agent role are exactly the six (skills.install.21).
    let dir = TempDir::new().unwrap();
    let listing = canon(dir.path(), &["schema", "--json"]);
    assert!(listing.status.success(), "{}", text(&listing));
    let listing: Value = serde_json::from_slice(&listing.stdout).unwrap();
    let closed: Vec<String> = listing["data"]["commands"].as_array().unwrap().iter().filter(|c| c["x-canon"]["roles"].as_array().is_some_and(Vec::is_empty)).map(|c| c["command"].as_str().unwrap().to_string()).collect();
    assert_eq!(human_only_problems(&closed), Vec::<String>::new());

    // The help of the leaves and of the two reads that take the new kind, and the generated spec.
    for leaf in ["defer", "drop", "reopen"] {
        let help = canon(dir.path(), &["change", leaf, "--help"]);
        let help = text(&help);
        for option in CLOSING_OPTIONS {
            assert!(help.contains(&format!("--{option}")), "`canon change {leaf} --help` lacks `--{option}`: {help}");
        }
    }
    for command in ["query", "search"] {
        let help = text(&canon(dir.path(), &[command, "--help"]));
        assert!(help.contains("change_state"), "`canon {command} --help` lacks the kind `change_state`: {help}");
    }
    let spec = canon_cli::spec::generate().to_string();
    for needle in ["change defer", "change drop", "change reopen", "change_state", "CHANGE_CONFLICT", "CHANGE_RECORDS_KEPT"] {
        assert!(spec.contains(needle), "the generated spec lacks `{needle}`");
    }

    // `canon init` writes the routing line.
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "seed"]);
    let init = canon(dir.path(), &["init", "--no-harness", "--no-agents-md"]);
    assert!(init.status.success(), "{}", text(&init));
    let yaml = std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap();
    assert!(yaml.lines().any(|line| line.trim() == "change_state: local"), "canon init writes `change_state: local`:\n{yaml}");

    // The CHANGELOG says it, too.
    assert_eq!(release_problems(&changelog()), Vec::<String>::new());
}

/// change.drop.25 (F8, failure path): a guide command line that does not fit the schema, a human-only list that is not six and a CHANGELOG without the
/// caller note each fail their check, and the failure names the line.
#[test]
fn change_drop_25_a_guide_line_off_the_schema_a_list_that_is_not_six_or_a_changelog_without_the_caller_note_fails() {
    let root = canon_cli::cli::command();

    // A guide line that does not fit the schema: an option the leaf does not have, a leaf that does not exist, a line without an option of the procedure.
    let real = closing_line(&flat(guide("role-planner"))).expect("the planner's closing line");
    assert_eq!(line_problems(&root, "role-planner", &real), Vec::<String>::new(), "{real}");
    let off = format!("{real} --bogus x");
    let problems = line_problems(&root, "role-planner", &off);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains(&off) && problems[0].contains("`--bogus` is not an option of `canon change defer`"), "{problems:?}");
    let unknown = real.replace("change defer", "change snooze");
    assert_eq!(line_problems(&root, "role-planner", &unknown), [format!("the guide `role-planner` line `{unknown}` names no leaf of the CLI")]);
    let without_relay = real.replace("--relayed-via <the question tool> ", "");
    assert_eq!(line_problems(&root, "role-planner", &without_relay), [format!("the guide `role-planner` line `{without_relay}` never gives `--relayed-via`")]);
    // A guide that lost the procedure, and one that still lists three leaves.
    let problems = guide_problems(&root, "canon-subject", "The human-only leaves are three: `brief approve`, `policy approve` and `divergence accept`.");
    for leaf in ["change drop", "change defer", "change reopen"] {
        assert!(problems.iter().any(|p| p.contains(&format!("`{leaf}`"))), "{leaf}: {problems:?}");
    }
    assert!(problems.iter().any(|p| p.contains("never says the human-only leaves are six")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("has no closing line")), "{problems:?}");

    // A human-only list that is not six: the leaves the listing names are three, five or seven.
    let six: Vec<String> = LEAVES.iter().map(ToString::to_string).collect();
    assert_eq!(human_only_problems(&six), Vec::<String>::new());
    let three = six[..3].to_vec();
    let problems = human_only_problems(&three);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("\"brief approve\", \"divergence accept\", \"policy approve\""), "the problem names the list: {problems:?}");
    let seven = [six.clone(), vec!["subject move".to_string()]].concat();
    assert_eq!(human_only_problems(&seven).len(), 1);

    // A CHANGELOG without the caller note, and without one of its four facts.
    let real = changelog();
    assert_eq!(release_problems(&real), Vec::<String>::new());
    assert_eq!(release_problems("# Changelog\n\n## Unreleased\n\n## 0.41.0\n"), ["the Unreleased section has no Added part", "the Unreleased section has no Changed part", "the Unreleased section has no `**Callers:**` note"]);
    assert_eq!(release_problems("# Changelog\n\n## 0.41.0\n"), ["the CHANGELOG has no Unreleased section"]);
    assert_eq!(release_problems(&real.replace("**Callers:**", "**Notes:**")), ["the Unreleased section has no `**Callers:**` note"]);
    let facts = [
        ("- A closed change drops out of the requirements.", "the callers note lacks the fact: a closed change drops out of the requirements"),
        ("`canon migrate` before the closing commands", "the callers note lacks the fact: a repo without the routing line runs `canon migrate` before the closing commands"),
        ("by hand", "the callers note lacks the fact: canon 0.41 and older refuse the canon.yaml with CONFIG_INVALID and deleting the `change_state:` line by hand goes back"),
        ("`!.canon/ledger/kind=change_state/`", "the callers note lacks the fact: a hand-written .gitignore adds `!.canon/ledger/kind=change_state/`"),
    ];
    for (needle, problem) in facts {
        assert!(real.contains(needle), "the CHANGELOG has `{needle}`");
        assert_eq!(release_problems(&real.replace(needle, "removed")), [problem], "without `{needle}`");
    }
    // An entry that lost a piece of Added or Changed is named by it.
    let problems = release_problems(&real.replace("`CHANGE_RECORDS_KEPT`", "a warning"));
    assert_eq!(problems, ["the Added part never says ``CHANGE_RECORDS_KEPT``"], "{problems:?}");
}
