//! `canon skills show` as atip/1 1.2 spec 14 has it (canon 0.35 `canon-0-35-skills-show`): `NAME` is optional and the default guide
//! lists every guide, `--json` is the contract's guide shape, an unknown name refuses with the guide names, the first line of every
//! `--help` points at `skills show` too. Tests are named by scenario
//! id (`atip.skillsshow.NN`).

#[path = "support/hermetic.rs"]
mod hermetic;
mod harness;
mod rule_support;

use std::collections::BTreeSet;
use std::path::Path;

use rule_support::run_empty_with;
use serde_json::{json, Value};

fn canon_in(dir: &Path, args: &[&str]) -> std::process::Output {
    hermetic::canon().args(args).current_dir(dir).env_clear().env("PATH", std::env::var_os("PATH").unwrap_or_default()).env("HOME", dir).output().expect("spawn canon")
}

fn json_of(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&output.stdout)))
}

fn guide_names() -> Vec<String> {
    canon_cli::guide::names().iter().map(ToString::to_string).collect()
}

/// Every file of a directory tree with its bytes, to compare a directory before and after a call.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).expect("read a directory").flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.push((path.strip_prefix(root).expect("inside").display().to_string(), std::fs::read(&path).unwrap_or_default()));
            }
        }
    }
    walk(dir, dir, &mut files);
    files
}

// ----------------------------------------------------------------------------------------------------------------- F1

/// atip.skillsshow.01: `canon skills show --json` without a name answers the default guide, `canon`, with every guide name in `guides`, in
/// an empty directory.
#[test]
fn atip_skillsshow_01_the_default_guide_lists_every_guide() {
    let empty = tempfile::tempdir().expect("an empty directory");
    let output = canon_in(empty.path(), &["skills", "show", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stdout));
    let document = json_of(&output);
    assert_eq!(document["ok"], true);
    assert_eq!(document["data"]["name"], "canon");
    let guides: Vec<String> = document["data"]["guides"].as_array().expect("guides").iter().map(|name| name.as_str().expect("a name").to_string()).collect();
    assert_eq!(guides, guide_names());
    assert!(!guides.is_empty());
    assert_eq!(document["data"]["text"].as_str(), canon_cli::guide::guide("canon"));
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    // The default guide's own text names every guide (spec 14.2).
    let text = document["data"]["text"].as_str().expect("text");
    let unnamed: Vec<&String> = guides.iter().filter(|name| !text.contains(name.as_str())).collect();
    assert!(unnamed.is_empty(), "the default guide does not name {unnamed:?}");
}

/// atip.skillsshow.02: the text of the call without a name is the default guide's Markdown byte for byte, and neither form writes
/// anything, in an empty directory or in one that holds files.
#[test]
fn atip_skillsshow_02_a_call_without_a_name_prints_the_default_guide_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::create_dir_all(dir.path().join(".canon")).expect("state");
    std::fs::write(dir.path().join("canon.yaml"), "project: x\n").expect("a file");
    let before = snapshot(dir.path());
    let bare = canon_in(dir.path(), &["skills", "show"]);
    let named = canon_in(dir.path(), &["skills", "show", "canon"]);
    assert_eq!(bare.status.code(), Some(0));
    assert_eq!(bare.stdout, named.stdout);
    assert_eq!(bare.stdout, canon_cli::guide::guide("canon").expect("the core guide").as_bytes());
    for args in [&["skills", "show", "--json"][..], &["skills", "show", "--full", "--json"], &["skills", "show", "role-planner", "--json"], &["skills", "show", "nosuch-xyz", "--json"]] {
        canon_in(dir.path(), args);
    }
    assert_eq!(snapshot(dir.path()), before, "no call of `skills show` writes");
    // Without a repository at all, and without a role, it answers the same.
    let empty = tempfile::tempdir().expect("an empty directory");
    assert_eq!(canon_in(empty.path(), &["skills", "show"]).stdout, bare.stdout);
    assert!(snapshot(empty.path()).is_empty());
}

// ----------------------------------------------------------------------------------------------------------------- F2

/// atip.skillsshow.03: every guide's `--json` is `{name, text, guides, full?}` and passes the contract's guide schema, `name` is the name
/// asked for, `x-canon.version` is canon's, and `--full` says so.
#[test]
fn atip_skillsshow_03_every_guide_passes_the_contracts_guide_schema() {
    let empty = tempfile::tempdir().expect("an empty directory");
    let mut names = guide_names();
    names.push(String::new());
    for name in &names {
        let mut args = vec!["skills", "show"];
        if !name.is_empty() {
            args.push(name);
        }
        args.push("--json");
        let document = json_of(&canon_in(empty.path(), &args));
        assert_eq!(document["ok"], true, "{name}: {document}");
        let data = &document["data"];
        assert_eq!(atip::guide_problems(data), Vec::<String>::new(), "`{name}`");
        assert_eq!(data["name"], if name.is_empty() { "canon" } else { name.as_str() });
        assert_eq!(data["x-canon"]["version"], env!("CARGO_PKG_VERSION"), "{name}");
        assert_eq!(data["full"], false, "{name}");
        assert_eq!(atip::envelope_problems(&document), Vec::<String>::new(), "{name}");
        // The command document says the same: the artifact is `SkillsShowReport`, which is the contract's guide schema.
        assert_eq!(document["meta"]["command"], "skills show");
        assert_eq!(document["meta"]["schemaVersion"], "2.0");
    }
    let full = json_of(&canon_in(empty.path(), &["skills", "show", "canon", "--full", "--json"]));
    assert_eq!(full["data"]["full"], true);
    assert_eq!(atip::guide_problems(&full["data"]), Vec::<String>::new());
    assert!(full["data"]["text"].as_str().expect("text").len() > canon_cli::guide::guide("canon").expect("guide").len());
    // `--full` on a guide that is complete already changes nothing, and `full` says nothing was added.
    let other = json_of(&canon_in(empty.path(), &["skills", "show", "role-planner", "--full", "--json"]));
    assert_eq!(other["data"]["full"], false);
    assert_eq!(other["data"]["text"].as_str(), canon_cli::guide::guide("role-planner"));
}

/// atip.skillsshow.04: the old members are gone, the contract's schema refuses a changed document, and the command document refers to the
/// schema by its `$id`.
#[test]
fn atip_skillsshow_04_the_old_members_are_gone_and_the_schema_is_held() {
    let empty = tempfile::tempdir().expect("an empty directory");
    let document = json_of(&canon_in(empty.path(), &["skills", "show", "canon", "--json"]));
    let data = document["data"].as_object().expect("data");
    assert!(!data.contains_key("markdown") && !data.contains_key("version"), "{:?}", data.keys().collect::<Vec<_>>());
    let keys: BTreeSet<&str> = data.keys().map(String::as_str).collect();
    assert_eq!(keys, BTreeSet::from(["name", "text", "guides", "full", "x-canon"]));
    // A document of the old shape, and a changed one, break the schema.
    let old = json!({ "name": "canon", "version": "0.34.0", "markdown": "# canon" });
    assert!(!atip::guide_problems(&old).is_empty());
    let mut broken = document["data"].clone();
    broken["guides"] = json!([]);
    assert!(!atip::guide_problems(&broken).is_empty(), "a guide list may not be empty");
    broken["guides"] = json!(["canon", "canon"]);
    assert!(!atip::guide_problems(&broken).is_empty(), "a guide name appears once");
    let mut upper = document["data"].clone();
    upper["name"] = json!("Canon");
    assert!(!atip::guide_problems(&upper).is_empty(), "a name is lower-case and hyphens");
    let mut extra = document["data"].clone();
    extra["version"] = json!("0.35.0");
    assert!(!atip::guide_problems(&extra).is_empty(), "only `x-` members are open");
    // The schema of the report is the contract's, by `$id`; one changed character of the vendored copy is caught by the digest.
    let model = canon_cli::spec::model();
    let model_doc = model["commands"].as_array().unwrap().iter().find(|doc| doc["command"] == "skills show").unwrap();
    let doc = serde_json::json!({ "data": model_doc });
    assert_eq!(doc["data"]["schemas"]["SkillsShowReport"]["$ref"], "https://atip.dev/1/guide.schema.json");
    assert_eq!(doc["data"]["outputs"][0]["kind"], "artifact");
    assert_eq!(doc["data"]["outputs"][0]["schemaVersion"], "2.0");
    assert_eq!(doc["data"]["operands"][0]["required"], false);
}

// ----------------------------------------------------------------------------------------------------------------- F3

/// atip.skillsshow.05: an unknown name, and a value outside the grammar of a name (an upper-case letter, a space), refuse with exit 2, the
/// registered `USAGE`, `field` `NAME` and `expected` the guide names; and a name that exists is not refused.
#[test]
fn atip_skillsshow_05_an_unknown_name_refuses_with_the_guide_names() {
    for value in ["nosuch-xyz", "Canon", "role implementer", "role_implementer", ""] {
        let outcome = run_empty_with(&["skills".to_string(), "show".to_string(), value.to_string()], &[]);
        assert_eq!(outcome.exit, 2, "`{value}`: {outcome:?}");
        assert_eq!(outcome.code.as_deref(), Some("USAGE"), "`{value}`");
        assert_eq!(outcome.field.as_deref(), Some("NAME"), "`{value}`");
        let document: Value = serde_json::from_str(&outcome.stdout).expect("a failure envelope");
        let expected: Vec<String> = document["error"]["expected"].as_array().expect("expected").iter().map(|name| name.as_str().expect("a name").to_string()).collect();
        assert_eq!(expected, guide_names(), "`{value}`");
        assert!(outcome.changed.is_empty());
    }
    let known = run_empty_with(&["skills".to_string(), "show".to_string(), "role-reviewer".to_string()], &[]);
    assert_eq!(known.exit, 0, "{known:?}");
}

// ----------------------------------------------------------------------------------------------------------------- F4

fn first_line(args: &[&str]) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    String::from_utf8_lossy(&canon_in(dir.path(), args).stdout).lines().next().unwrap_or_default().to_string()
}

/// atip.skillsshow.06: the first line of the help of the root and of a group and a command names `schema` and `skills show`, and the
/// introduction stays below it.
#[test]
fn atip_skillsshow_06_the_first_line_of_every_help_points_at_schema_and_skills_show() {
    for args in [&["--help"][..], &["finding", "--help"], &["finding", "close", "--help"], &["skills", "show", "--help"], &["help", "finding"], &["schema", "-h"]] {
        let line = first_line(args);
        assert!(line.contains("schema") && line.contains("skills show"), "{args:?}: {line}");
    }
    let mut root = canon_cli::cli::command();
    root.build();
    for node in atip::project::nodes(&root) {
        let help = node.cmd.clone().render_long_help().to_string();
        let first = help.lines().next().unwrap_or_default();
        assert!(first.contains("schema") && first.contains("skills show"), "`{}`: {first}", node.command);
    }
}

/// Whether a first line of help points at both entrances, `schema` and `skills show`.
fn points_at_both(line: &str) -> bool {
    line.contains("schema") && line.contains("skills show")
}

/// atip.skillsshow.10 (the failure path of F4): a first line that names only `schema` (as 0.34 had it), only `skills show`, or neither is
/// not the pointer; and a help whose pointer line is cut is read from the line below it, which is not the pointer either.
#[test]
fn atip_skillsshow_10_a_first_line_that_names_one_entrance_or_none_is_not_the_pointer() {
    let mut root = canon_cli::cli::command();
    root.build();
    for node in atip::project::nodes(&root) {
        let help = node.cmd.clone().render_long_help().to_string();
        let mut lines = help.lines();
        assert!(points_at_both(lines.next().unwrap_or_default()), "`{}`: the first line is the pointer", node.command);
        let below = lines.find(|line| !line.trim().is_empty()).unwrap_or_default();
        assert!(!points_at_both(below), "`{}`: the line below the pointer is the introduction, not a second pointer: {below}", node.command);
    }
}
