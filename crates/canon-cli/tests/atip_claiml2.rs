//! `canon-0-35-claim-l2`, moved to 1.4 by `canon-0-36-claim-1-4`: canon declares atip/1 1.4 and claims level L2, and the CI job proves it (scenarios `atip.claiml2.01-18`; `canon-0-36-atip-dep` moved the checker to the atip repository).
//!
//! The declaration is `canon_cli::json_out::DECLARED_MINOR` and `canon_cli::spec::LEVEL`; the proof is `atip check --level L2 --contract 1.4 --deep`
//! on the committed fixture, which `just atip-deep` and the job `atip` run (`just atip` runs it without `--deep`), and which no test of canon runs. The CHANGELOG holds the release entry and the list of tests that
//! stand for the obligations of contract 17.2 that a green checker run does not prove; the last four tests read it.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

const CANON: &str = env!("CARGO_BIN_EXE_canon");

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/atip/canon.fixture.json")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The exit status and the one JSON document of `canon <args>` run in `dir`.
fn canon_out(dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let output = Command::new(CANON).args(args).current_dir(dir).env_remove("CANON_ROLE").output().expect("spawn canon");
    let document = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{args:?}: {error}: {}", String::from_utf8_lossy(&output.stdout)));
    (output.status.code(), document)
}

fn canon_in(dir: &Path, args: &[&str]) -> Value {
    canon_out(dir, args).1
}

#[test]
fn atip_claiml2_01_every_envelope_declares_1_2_and_the_schema_tree_says_l2() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(canon_cli::json_out::DECLARED_MINOR, "1.4");
    assert_eq!(canon_cli::spec::LEVEL, "L2");
    let init = Command::new(CANON).args(["init", "--providers", "none"]).current_dir(dir.path()).env_remove("CANON_ROLE").output().expect("init");
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    for args in [&["status", "--json"][..], &["nosuch", "--json"], &["schema", "--json"], &["schema", "--errors", "--json"], &["schema", "--warnings", "--json"]] {
        let document = canon_in(dir.path(), args);
        assert_eq!(document["meta"]["contractVersion"], "1.4", "{args:?}: {document}");
        assert_eq!(atip::envelope_problems(&document), Vec::<String>::new(), "{args:?}");
    }
    let schema = canon_in(dir.path(), &["schema", "--json"]);
    assert_eq!((schema["data"]["kind"].as_str(), schema["data"]["level"].as_str(), schema["meta"]["schemaVersion"].as_str()), (Some("tree"), Some("L2"), Some("1.4")), "{schema}");
}

/// Whether `text` says that canon's `contractVersion` is one of the minors it no longer declares (`1.1`, `1.2`, `1.3`), in any of the ways the repository writes it (JSON, escaped JSON, code spans, prose).
fn spells_an_old_minor(text: &str) -> bool {
    text.match_indices("contractVersion").any(|(at, word)| {
        let after: String = text[at + word.len()..].chars().take(12).filter(|c| !matches!(c, '"' | '\\' | ':' | '`' | '(' | ' ' | '=' | '\'')).collect();
        ["1.1", "1.2", "1.3"].iter().any(|old| after.strip_prefix(old).is_some_and(|rest| !rest.starts_with(|c: char| c == '.' || c.is_ascii_digit())))
    })
}

#[test]
fn atip_claiml2_02_no_consumer_spells_canons_declaration_as_1_1() {
    assert!(spells_an_old_minor("\"contractVersion\": \"1.1\"") && spells_an_old_minor("`contractVersion` (`1.2`, on every envelope)") && spells_an_old_minor("{\\\"contractVersion\\\":\\\"1.3\\\"}"), "the detector sees the spellings");
    assert!(!spells_an_old_minor("\"contractVersion\": \"1.4\"") && !spells_an_old_minor("`contractVersion` (`1.4`)") && !spells_an_old_minor("contractVersion 1.9") && !spells_an_old_minor("contractVersion 1.2.1"));
    let root = repo_root();
    let mut found = Vec::new();
    // What reads canon's declaration: the skills, the website, the eval scripts, the conformance corpus, the generated spec and canon's own sources.
    for place in ["canon/skills", "packages/website/src", "packages/cli", "packages/dashboard/src", "packages/dashboard/test", "evals", "conformance", "docs", "crates/canon-cli/src"] {
        let mut pending = vec![root.join(place)];
        while let Some(path) = pending.pop() {
            if path.file_name().is_some_and(|name| name == "node_modules" || name == "target") {
                continue;
            }
            if path.is_dir() {
                pending.extend(std::fs::read_dir(&path).expect("a directory").map(|entry| entry.expect("an entry").path()));
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                if spells_an_old_minor(&text) {
                    found.push(path.strip_prefix(&root).unwrap_or(&path).display().to_string());
                }
            }
        }
    }
    found.sort();
    assert!(found.is_empty(), "these files spell canon's contractVersion as 1.1, 1.2 or 1.3: {found:?}");
}

#[test]
fn atip_claiml2_06_input_too_large_names_the_field_input() {
    let dir = tempfile::tempdir().expect("tempdir");
    let init = Command::new(CANON).args(["init", "--providers", "none"]).current_dir(dir.path()).env_remove("CANON_ROLE").output().expect("init");
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let big = dir.path().join("big.json");
    std::fs::write(&big, vec![b' '; 8 * 1024 * 1024 + 1]).expect("write");
    let (bytes_exit, bytes) = canon_out(dir.path(), &["scenario", "new", "--input", big.to_str().expect("utf8"), "--json"]);
    let items = format!("[{}]", vec!["{}"; 10_001].join(","));
    let (many_exit, many) = canon_out(dir.path(), &["scenario", "new", "--input", &items, "--json"]);
    for (exit, document, reason) in [(bytes_exit, &bytes, "TOO_MANY_BYTES"), (many_exit, &many, "TOO_MANY_ITEMS")] {
        assert_eq!(exit, Some(2), "a refusal: {document}");
        assert_eq!((document["error"]["code"].as_str(), document["error"]["reason"].as_str(), document["error"]["field"].as_str()), (Some("INPUT_TOO_LARGE"), Some(reason), Some("--input")), "{document}");
    }
}

/// The `run:` lines of the job `job` of the workflow.
fn job_commands(workflow: &str, job: &str) -> Vec<String> {
    let mut lines = workflow.lines().skip_while(|line| *line != format!("  {job}:"));
    assert!(lines.next().is_some(), "the job {job} is in the workflow");
    lines.take_while(|line| !line.starts_with("  ") || line.starts_with("   ")).filter_map(|line| line.trim().strip_prefix("run: ")).map(str::to_string).collect()
}

/// The command lines of the recipe `name` of the justfile, `{{ target }}` spelled `target`.
fn recipe_lines(justfile: &str, name: &str) -> Vec<String> {
    let mut lines = justfile.lines().skip_while(|line| !line.starts_with(&format!("{name}:")));
    assert!(lines.next().is_some(), "the recipe {name} is in the justfile");
    lines.take_while(|line| line.starts_with("    ")).map(|line| line.trim().replace("{{ target }}", "target")).collect()
}

#[test]
fn atip_claiml2_07_just_atip_and_the_job_run_canon_at_l2_with_deep_at_1_2() {
    let root = repo_root();
    let workflow = read(&root.join(".github/workflows/test.yml"));
    let justfile = read(&root.join("justfile"));
    let canon_at_l2 = "atip check --bin target/debug/canon --level L2 --contract 1.4 --deep --fixture crates/canon-cli/tests/atip/canon.fixture.json".to_string();
    let recipe = recipe_lines(&justfile, "atip-deep");
    assert!(recipe.contains(&canon_at_l2), "the recipe atip-deep runs `{canon_at_l2}`: {recipe:?}");
    assert!(recipe.iter().any(|line| line.contains("cargo build") && line.contains("-p canon-cli") && !line.contains("atip-check")), "the recipe builds canon only: {recipe:?}");
    assert!(!recipe.iter().any(|line| line.contains("--level L0") || line.contains("--level L1")), "the L2 report holds the L0 and L1 rows: {recipe:?}");
    let quick = recipe_lines(&justfile, "atip");
    assert!(quick.contains(&canon_at_l2.replace(" --deep", "")), "the recipe atip runs the same check without --deep: {quick:?}");
    let job = job_commands(&workflow, "atip");
    assert!(job.contains(&canon_at_l2), "the job atip runs `{canon_at_l2}`: {job:?}");
    assert!(job.iter().any(|line| line.contains("cargo build") && line.contains("-p canon-cli") && !line.contains("atip-check")), "{job:?}");
    for retired in ["atip-l0", "atip-l1", "atip-l2", "atip-check-tests"] {
        assert!(!workflow.lines().any(|line| line == format!("  {retired}:")), "the job {retired} is folded into the job atip");
    }
    let ci = justfile.lines().find(|line| line.starts_with("ci:")).expect("the recipe ci");
    assert!(ci.split_whitespace().any(|word| word == "atip-deep"), "just ci runs the recipe atip-deep, as the job does: {ci}");
}

#[test]
fn atip_claiml2_09_the_fixture_claims_1_2_and_lists_what_the_l2_rows_need() {
    let fixture: Value = serde_json::from_str(&read(&fixture_path())).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], "1.4");
    assert_eq!(fixture["notRun"], json!(["dashboard"]));
    let prepare = fixture["prepare"].as_array().expect("prepare");
    assert_eq!(prepare[0], json!(["git", "init", "-q"]));
    assert!(prepare[1].as_array().is_some_and(|step| step.iter().any(|word| word == "--allow-empty")), "an empty commit: {prepare:?}");
    let commands = fixture["commands"].as_object().expect("commands");
    for id in ["purge", "artifacts move", "migrate", "ledger compact", "tier age"] {
        assert!(commands[id]["preview"].as_array().is_some_and(|previews| !previews.is_empty()), "the preview of {id}");
    }
    for id in ["export", "finding close"] {
        for key in ["ok", "batchFailsAt", "batchFailsFirst"] {
            assert!(commands[id][key].as_array().is_some_and(|cases| !cases.is_empty()), "{id}.{key}");
        }
    }
    for id in ["export", "query"] {
        assert_eq!(commands[id]["allowedEffects"], json!([".canon/audit/access.jsonl"]), "{id}");
    }
    // The destructive commands the schema lists are the ones the fixture previews.
    let dir = tempfile::tempdir().expect("tempdir");
    let schema = canon_in(dir.path(), &["schema", "--json"]);
    let mut destructive: Vec<String> = schema["data"]["commands"].as_array().expect("commands").iter().filter(|entry| entry["effect"] == "destructive").map(|entry| entry["command"].as_str().expect("a name").to_string()).collect();
    destructive.sort();
    let mut previewed: Vec<String> = commands.iter().filter(|(_, entry)| entry.get("preview").is_some()).map(|(id, _)| id.clone()).collect();
    previewed.sort();
    assert_eq!(previewed, destructive, "every destructive command has a preview in the fixture");
}

/// The part of the CHANGELOG that is the release being prepared (the section `## 0.35.0`).
fn release_section(changelog: &str) -> &str {
    let start = changelog.find("\n## 0.35.0").expect("the CHANGELOG has the release section");
    let rest = &changelog[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |at| at + 3);
    &rest[..end]
}

/// The obligations of `approaches/checker.md` section 8 (atip/1 1.2.1 and 1.3.1: 18 rows), each by the words its row of the CHANGELOG list opens with.
const OBLIGATIONS: [&str; 18] = [
    "tree complete",
    "exit codes registered",
    "write constraints",
    "preview equals commit",
    "remote effects of reads and refusals",
    "secrets",
    "commit atomicity",
    "operation keys",
    "streams cut off",
    "examples and guides parse",
    "no /dev/tty prompt",
    "next syntax",
    "L0 paging",
    "domains and validators",
    "conservative staging",
    "reasons registered",
    "stopped midway",
    "warning codes",
];

/// What is wrong with the test `name` of the source file `file` (a path under `crates`): the file is missing, the function is not there, it is not a
/// `#[test]`, or it is `#[ignore]`d, which keeps it out of the workspace run. The attributes and doc comments directly above the function are read.
fn test_problem(crates: &Path, file: &str, name: &str) -> Option<String> {
    let Ok(source) = std::fs::read_to_string(crates.join(file)) else { return Some(format!("{file} does not exist")) };
    let lines: Vec<&str> = source.lines().collect();
    let Some(at) = lines.iter().position(|line| line.trim_start().starts_with(&format!("fn {name}(")) || line.trim_start().starts_with(&format!("pub fn {name}("))) else {
        return Some(format!("{file} has no function `{name}`"));
    };
    let mut attributes: Vec<&str> = Vec::new();
    for line in lines[..at].iter().rev() {
        let line = line.trim();
        if line.starts_with("#[") {
            attributes.push(line);
        } else if !line.starts_with("///") {
            break;
        }
    }
    if !attributes.contains(&"#[test]") {
        return Some(format!("`{name}` in {file} is not a #[test]"));
    }
    attributes.iter().find(|attribute| attribute.starts_with("#[ignore")).map(|_| format!("`{name}` in {file} is #[ignore]d: it is not in the workspace run"))
}

/// What is wrong with the evidence list of `section`: an obligation without a row, a row with neither a test nor a stated `N/A:` or `Gap:`, a test that is
/// not a live `#[test]` of the sources.
fn evidence_problems(section: &str, crates: &Path) -> Vec<String> {
    let Some(start) = section.find("**Evidence of contract 17.2**") else { return vec!["the section has no evidence list".to_string()] };
    let table: Vec<&str> = section[start..].lines().skip_while(|line| !line.starts_with('|')).take_while(|line| line.starts_with('|')).collect();
    let mut problems = Vec::new();
    for obligation in OBLIGATIONS {
        let Some(row) = table.iter().find(|line| line.starts_with(&format!("| {obligation} |"))) else {
            problems.push(format!("no row for `{obligation}`"));
            continue;
        };
        let tests: Vec<&str> = row.split('`').skip(1).step_by(2).filter(|word| word.contains("::")).collect();
        if tests.is_empty() && !row.contains("N/A: ") && !row.contains("Gap: ") {
            problems.push(format!("`{obligation}` names no test, no `N/A:` and no `Gap:`"));
        }
        for test in tests {
            let Some((file, name)) = test.split_once("::") else { continue };
            if let Some(problem) = test_problem(crates, file, name) {
                problems.push(format!("`{obligation}`: {problem}"));
            }
        }
    }
    problems
}

#[test]
fn atip_claiml2_11_every_obligation_of_contract_17_2_has_a_row_with_live_tests_or_a_stated_reason() {
    let root = repo_root();
    let changelog = read(&root.join("CHANGELOG.md"));
    assert_eq!(evidence_problems(release_section(&changelog), &root.join("crates")), Vec::<String>::new());
}

#[test]
fn atip_claiml2_12_a_row_taken_out_of_the_list_or_a_test_that_is_not_live_fails_the_check() {
    let root = repo_root();
    let changelog = read(&root.join("CHANGELOG.md"));
    let section = release_section(&changelog);
    for obligation in OBLIGATIONS {
        let without: String = section.lines().filter(|line| !line.starts_with(&format!("| {obligation} |"))).map(|line| format!("{line}\n")).collect();
        let problems = evidence_problems(&without, &root.join("crates"));
        assert_eq!(problems, [format!("no row for `{obligation}`")], "{obligation}");
    }
    let renamed = section.replace("every_node_of_the_clap_tree_is_in_the_spec_and_the_reverse", "a_test_nobody_wrote");
    let problems = evidence_problems(&renamed, &root.join("crates"));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("a_test_nobody_wrote"), "{problems:?}");
    // A row that gives neither a test nor a reason fails, and so does a test that is ignored, not a #[test], or in a file that is not there.
    let bare: String = section.lines().map(|line| if line.starts_with("| operation keys |") { "| operation keys | nothing |".to_string() } else { line.to_string() }).map(|line| format!("{line}\n")).collect();
    assert_eq!(evidence_problems(&bare, &root.join("crates")), ["`operation keys` names no test, no `N/A:` and no `Gap:`"]);
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("t.rs"), "#[test]\n#[ignore]\nfn ignored() {}\n\n/// doc\n#[test]\nfn live() {}\n\nfn plain() {}\n").expect("write");
    assert_eq!(test_problem(dir.path(), "t.rs", "live"), None);
    assert!(test_problem(dir.path(), "t.rs", "ignored").is_some_and(|problem| problem.contains("#[ignore]")));
    assert!(test_problem(dir.path(), "t.rs", "plain").is_some_and(|problem| problem.contains("not a #[test]")));
    assert!(test_problem(dir.path(), "t.rs", "absent").is_some_and(|problem| problem.contains("no function")));
    assert!(test_problem(dir.path(), "nofile.rs", "live").is_some_and(|problem| problem.contains("does not exist")));
}

/// The entries the 0.35.0 section carries for the changes of the release: a name, and the opening words of the entry, which only that entry has.
const RELEASE_ENTRIES: [(&str, &str); 23] = [
    ("the declaration 1.2 and level L2", "**canon declares atip/1 1.2 and claims level L2**"),
    ("canon init never asks", "**`canon init` never asks**"),
    ("the CI jobs that prove the claim", "**CI proves the claim.**"),
    ("the evidence list of contract 17.2", "**Evidence of contract 17.2**"),
    ("the answer order of the stateless checks", "**The stateless checks of every command answer in the order of atip/1 1.2**"),
    ("--input in the order of spec 11.7", "**`--input` follows the order of spec 11.7**"),
    ("a # line of an NDJSON file is NOT_JSON", "a `#` line of an NDJSON file was skipped as a comment"),
    ("a value outside a set in a document is INVALID_VALUE", "a value outside a closed set in a document was `USAGE`"),
    ("+1 on argv is the integer 1", "`--round +1` against `1`"),
    ("the pointers and outcomes of a batch", "**Batches follow spec 11.8 and 11.11**"),
    ("the preflight of a batch runs no --dry-run", "the 14 commands that have `--dry-run`"),
    ("the guide command skills show", "**`canon skills show` is the contract's guide command**"),
    ("NAME of skills show is optional", "`NAME` is optional: without it the command prints the default guide"),
    ("the first line of --help names skills show", "first line of `--help`: `Agents"),
    ("STOPPED_MIDWAY and the moved codes", "**canon registers `STOPPED_MIDWAY`"),
    ("the cursor and the flag-only options", "**The cursor is `atip::cursor`'s"),
    ("a cursor of 0.34 is refused", "Upgrade note: **a cursor 0.34 issued is refused"),
    ("the plan digest is sha256: and the JCS of effects and inputs", "**The digest of a destructive plan is `sha256:`"),
    ("a preview of 0.34 is PLAN_STALE", "**Upgrade note.** A preview made by 0.34 is `PLAN_STALE`"),
    ("a read and a refusal write nothing", "- **A refused call writes nothing, whatever the scenario index**"),
    ("INPUT_TOO_LARGE names --input", "| `INPUT_TOO_LARGE` (a source over 8 MiB"),
    ("an all-unchanged batch has only items", "**Upgrade note:** read `data.items[].status`"),
    ("the checker declares 1.2 and runs L2", "- **`atip check --level L2` runs, and `LEVEL_UNSUPPORTED` is gone.**"),
];

fn release_problems(section: &str) -> Vec<String> {
    RELEASE_ENTRIES
        .iter()
        .filter_map(|(name, opening)| match section.matches(opening).count() {
            1 => None,
            0 => Some(format!("the release section has no entry for: {name}")),
            _ => Some(format!("the release section says the opening of the entry more than once for: {name}")),
        })
        .collect()
}

#[test]
fn atip_claiml2_13_the_release_section_carries_every_change_of_the_release_once_and_the_evidence_list() {
    let changelog = read(&repo_root().join("CHANGELOG.md"));
    let section = release_section(&changelog);
    assert_eq!(release_problems(section), Vec::<String>::new());
    assert!(section.contains("**Evidence of contract 17.2**"), "the list stands in the release section");
    assert!(section.contains("Upgrade note"), "the upgrade notes stand in the release section");
}

#[test]
fn atip_claiml2_14_an_entry_taken_out_of_the_release_section_fails_the_check() {
    let changelog = read(&repo_root().join("CHANGELOG.md"));
    let section = release_section(&changelog);
    for (name, opening) in RELEASE_ENTRIES {
        let problems = release_problems(&section.replace(opening, "(taken out)"));
        assert_eq!(problems, [format!("the release section has no entry for: {name}")], "{name}");
    }
    // An opening that another text repeats does not pin its entry.
    let doubled = format!("{section}\n{}", RELEASE_ENTRIES[0].1);
    assert_eq!(release_problems(&doubled).len(), 1);
}

/// A repository with one commit, canon initialised, and a finding `add-login` 1/1 that is open.
fn repo_with_a_finding() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(dir.path()).output().expect("git").status.success(), "git {args:?}");
    git(&["init", "-q"]);
    git(&["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-qm", "fixture"]);
    for args in [
        &["init", "--providers", "none", "--json"][..],
        &["subject", "new", "alpha", "--domain", "core", "--title", "Alpha", "--json"],
        &["change", "new", "add-login", "--subject", "alpha", "--title", "Add login", "--json"],
        &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "fixture", "--summary", "first", "--json"],
    ] {
        let document = canon_in(dir.path(), args);
        assert_eq!(document["ok"], true, "{args:?}: {document}");
    }
    dir
}

#[test]
fn atip_claiml2_15_a_batch_of_unchanged_elements_has_only_items_in_data() {
    let dir = repo_with_a_finding();
    let close = json!({"operands": ["add-login"], "round": 1, "seq": 1, "disposition": "fixed"});
    let (first_exit, first) = canon_out(dir.path(), &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "fixed", "--json"]);
    assert_eq!((first_exit, first["ok"].clone()), (Some(0), json!(true)), "{first}");
    for count in [1, 2] {
        let batch = Value::Array(vec![close.clone(); count]).to_string();
        let (exit, document) = canon_out(dir.path(), &["finding", "close", "--input", &batch, "--json"]);
        assert_eq!(exit, Some(0), "{document}");
        assert_eq!(document["ok"], true, "{document}");
        let data = document["data"].as_object().expect("data is an object");
        assert_eq!(data.keys().collect::<Vec<_>>(), ["items"], "the batch report is closed: {document}");
        let items = data["items"].as_array().expect("items");
        assert_eq!(items.len(), count);
        for item in items {
            assert_eq!(item["status"], "unchanged", "{document}");
            assert_eq!(item["data"]["unchanged"], true, "an element carries its ordinary report: {document}");
        }
        assert_eq!(atip::envelope_problems(&document), Vec::<String>::new());
    }
}

#[test]
fn atip_claiml2_16_the_preview_of_a_move_to_the_local_store_leads_with_the_commit_call() {
    let dir = repo_with_a_finding();
    let (preview_exit, preview) = canon_out(dir.path(), &["artifacts", "move", "--tier", "local", "--json"]);
    assert_eq!(preview_exit, Some(0), "a preview exits 0: {preview}");
    assert_eq!((preview["ok"].clone(), preview["data"]["committed"].clone()), (json!(true), json!(false)), "{preview}");
    let digest = preview["data"]["planDigest"].as_str().expect("a digest");
    let argv: Vec<&str> = preview["meta"]["next"][0]["argv"].as_array().expect("meta.next[0].argv").iter().map(|word| word.as_str().expect("a word")).collect();
    assert_eq!(argv.first(), Some(&"artifacts"), "{preview}");
    assert!(argv.contains(&"--commit") && argv.contains(&"--json") && argv.contains(&format!("--plan-digest={digest}").as_str()), "{argv:?}");
    let (commit_exit, commit) = canon_out(dir.path(), &argv);
    assert_eq!(commit_exit, Some(0), "{commit}");
    assert_eq!((commit["ok"].clone(), commit["data"]["committed"].clone()), (json!(true), json!(true)), "{commit}");
}

/// The secret every output and file is searched for in `atip_claiml2_17`: the password of a Postgres DSN, the secret key of the s3 tier.
const SECRET: &str = "s3cr3t-Marker-9f2c41";

/// Every file under `dir`, as text where it is UTF-8, bytes searched where it is not.
fn files_holding(dir: &Path, needle: &str, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files_holding(&path, needle, found);
        } else if std::fs::read(&path).is_ok_and(|bytes| bytes.windows(needle.len()).any(|window| window == needle.as_bytes())) {
            found.push(path);
        }
    }
}

#[test]
fn atip_claiml2_17_a_known_secret_of_the_environment_reaches_no_output_and_no_file() {
    // The tiers canon.yaml can route to, named by the environment variables that carry their credentials: Postgres (a DSN with a password) and s3.
    const CANON_YAML: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: \"canon/\" }\nrouting:\n  change: local\n  task: hot\n  session: hot\n  trajectory: cold\n";
    // The search sees a marker where one is: a file that holds it is found.
    let planted = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(planted.path().join("sub")).expect("mkdir");
    std::fs::write(planted.path().join("sub/leak"), format!("x{SECRET}y")).expect("write");
    let mut seen = Vec::new();
    files_holding(planted.path(), SECRET, &mut seen);
    assert_eq!(seen, [planted.path().join("sub/leak")]);
    let dsn = format!("postgres://canon:{SECRET}@127.0.0.1:1/canon_v1");
    let environment = [
        ("CANON_PG_DSN", dsn.as_str()),
        ("DATABASE_URL", dsn.as_str()),
        ("CANON_R2_BUCKET", "canon-bucket"),
        ("CANON_S3_ENDPOINT", "http://127.0.0.1:1"),
        ("CANON_S3_ACCESS_KEY", "AKIA-access-s3cr3t-Marker-9f2c41"),
        ("CANON_S3_SECRET_KEY", SECRET),
    ];
    let calls: [&[&str]; 14] = [
        &["query", "--kind", "task"],
        &["query", "--kind", "trajectory"],
        &["query", "--kind", "change"],
        &["status"],
        &["gate", "check"],
        &["search", "x"],
        &["export", "--kind", "task"],
        &["purge", "--kind", "session", "--before", "2030-01-01"],
        &["purge", "--kind", "session", "--before", "2030-01-01", "--commit", "--plan-digest", "sha256:0000000000000000000000000000000000000000000000000000000000000000"],
        &["tier", "age"],
        &["artifacts", "move", "--tier", "cold"],
        &["artifacts", "verify"],
        &["ledger", "compact"],
        &["migrate"],
    ];
    // A call that reaches Postgres waits out the connection (30 seconds), so the calls run side by side, each in a repository of its own.
    let results: Vec<(String, bool, Vec<PathBuf>)> = std::thread::scope(|scope| {
        let mut running = Vec::new();
        for call in calls {
            for json_mode in [false, true] {
                let environment = &environment;
                running.push(scope.spawn(move || {
                    let dir = tempfile::tempdir().expect("tempdir");
                    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(dir.path()).output().expect("git").status.success(), "git {args:?}");
                    git(&["init", "-q"]);
                    git(&["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-qm", "fixture"]);
                    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).expect("write canon.yaml");
                    let mut command = Command::new(CANON);
                    command.args(call).current_dir(dir.path()).env_remove("CANON_ROLE").env_remove("CANON_R2_LOCAL_ROOT");
                    if json_mode {
                        command.arg("--json");
                    }
                    for (name, value) in environment {
                        command.env(name, value);
                    }
                    let output = command.output().expect("spawn canon");
                    let (out, err) = (String::from_utf8_lossy(&output.stdout).into_owned(), String::from_utf8_lossy(&output.stderr).into_owned());
                    let leaked = out.contains(SECRET) || err.contains(SECRET);
                    let named_a_tier = ["postgres", "s3"].iter().any(|tier| out.contains(tier) || err.contains(tier));
                    let mut found = Vec::new();
                    files_holding(dir.path(), SECRET, &mut found);
                    (format!("{call:?} json={json_mode}{}", if leaked { " printed the secret" } else { "" }), named_a_tier, found)
                }));
            }
        }
        running.into_iter().map(|handle| handle.join().expect("a probe")).collect()
    });
    let leaks: Vec<&str> = results.iter().filter(|(label, _, _)| label.ends_with("printed the secret")).map(|(label, _, _)| label.as_str()).collect();
    assert_eq!(leaks, Vec::<&str>::new(), "answers that print the secret");
    // The calls reached the tiers (probes that never connect would prove nothing), and nothing they wrote holds the secret either.
    let named = results.iter().filter(|(_, named_a_tier, _)| *named_a_tier).count();
    assert!(named >= 6, "{named} of {} answers name a remote tier: the secret was never in play", results.len());
    let holders: Vec<&PathBuf> = results.iter().flat_map(|(_, _, found)| found).collect();
    assert_eq!(holders, Vec::<&PathBuf>::new(), "files that hold the secret");
}

#[test]
fn atip_claiml2_18_no_command_of_canon_reads_a_terminal_for_input_and_init_asks_nothing_on_one() {
    fn sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push((path.clone(), read(&path)));
            }
        }
    }
    let mut found = Vec::new();
    for krate in std::fs::read_dir(repo_root().join("crates")).expect("crates") {
        let src = krate.expect("a crate").path().join("src");
        if src.is_dir() {
            sources(&src, &mut found);
        }
    }
    assert!(found.len() > 100, "the scan found canon's sources: {}", found.len());
    let mut offenders: Vec<String> = Vec::new();
    for (path, text) in &found {
        for needle in ["/dev/tty", "rpassword", "dialoguer", "inquire::", "read_password", "termios"] {
            // The scan is for the tool that is checked: every crate of this workspace.
            if text.lines().any(|line| line.contains(needle) && !line.trim_start().starts_with("//")) {
                offenders.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert_eq!(offenders, Vec::<String>::new(), "a source opens the controlling terminal or uses a prompt library");
    // Every read of the process's stdin, a path to it and every inherited stdio of a child: `--input -` alone, which refuses a terminal (`STDIN_TTY`)
    // instead of waiting. A new reader is a new prompt until the contract's 5.5 is met by it and it is named here.
    let mut readers: Vec<String> = found
        .iter()
        .filter(|(_, text)| text.lines().any(|line| !line.trim_start().starts_with("//") && (line.contains("io::stdin()") || line.contains("/dev/stdin") || line.contains("Stdio::inherit"))))
        .map(|(path, _)| path.strip_prefix(repo_root().join("crates")).unwrap_or(path).display().to_string().replace("/../", "/"))
        .collect();
    readers.sort();
    assert_eq!(readers, ["canon-cli/src/input.rs"], "a read of stdin that is not `--input -`");
    let input = read(&repo_root().join("crates/canon-cli/src/input.rs"));
    assert!(input.contains("STDIN_TTY") && input.contains("is_terminal()"), "`--input -` on a terminal is a refusal");
    // And running it: `canon init`, the one command that once asked, run with a terminal on stdin and stderr (a pty from `script`) prints no question and
    // installs for the detected harness. `init.rs` holds no prompt and nothing else reads a line.
    let init = read(&repo_root().join("crates/canon-cli/src/init.rs"));
    assert!(!init.contains("read_line") && !init.contains("prompt_providers"), "init asks nothing");
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join(".claude")).expect("mkdir");
    let mut command = Command::new("script");
    if cfg!(target_os = "macos") {
        command.args(["-q", "/dev/null", CANON, "init"]);
    } else {
        command.args(["-qec", &format!("{} init", atip::quote(CANON)), "/dev/null"]);
    }
    // The pipe stays open while canon runs: a prompt would wait on it, and this test would not end.
    let mut child = command.current_dir(dir.path()).env_remove("CANON_ROLE").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().expect("`script` gives canon a terminal; install it to run this case");
    let _held_open = child.stdin.take();
    let output = child.wait_with_output().expect("canon init on a terminal");
    let shown = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "{shown}");
    assert!(!shown.contains("Enter for") && !shown.contains("which harnesses") && !shown.contains("Comma-separated"), "init asked on a terminal: {shown}");
    assert!(read(&dir.path().join("canon.yaml")).ends_with("skills:\n  providers: [claude]\n"), "init took the detected harness");
}
