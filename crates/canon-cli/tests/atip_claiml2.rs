//! `canon-0-35-claim-l2`: canon declares atip/1 1.2 and claims level L2, and the CI jobs prove it (scenarios `atip.claiml2.01-14`).
//!
//! The declaration is `canon_cli::json_out::DECLARED_MINOR` and `canon_cli::spec::LEVEL`; the proof is `atip check --level L2 --contract 1.2`
//! on the committed fixture, which `just atip` and the job `atip-l2` run too. The CHANGELOG holds the release entry and the list of tests that
//! stand for the obligations of contract 17.2 that a green checker run does not prove; the last four tests read it.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use atip_check::app::check_binary;
use atip_check::cli::{CheckArgs, Level};
use atip_check::report::{CheckResult, Report, Strength};
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

fn canon_in(dir: &Path, args: &[&str]) -> Value {
    let output = Command::new(CANON).args(args).current_dir(dir).env_remove("CANON_ROLE").output().expect("spawn canon");
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{args:?}: {error}: {}", String::from_utf8_lossy(&output.stdout)))
}

fn run(bin: &Path, level: Level, fixture: &Path) -> Report {
    let args = CheckArgs { bin: bin.into(), level, fixture: fixture.into(), contract: Some("1.2".to_string()) };
    check_binary(&args).unwrap_or_else(|failure| panic!("{}: {}", failure.code.info().code, failure.message))
}

/// The ids of the MUST rows that are neither a pass nor unsupported.
fn red_musts(report: &Report) -> Vec<&'static str> {
    report.checks.iter().filter(|check| check.strength == Strength::Must && !matches!(check.result, CheckResult::Pass | CheckResult::Unsupported)).map(|check| check.id).collect()
}

/// A script that runs canon, and `rewrite` on what it prints; `dashboard` (it serves until it is signalled) is run as it is.
fn wrapper(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    let script = format!("#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = dashboard ] && exec '{CANON}' \"$@\"; done\n{body}");
    std::fs::write(&path, script).expect("write the wrapper");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[test]
fn atip_claiml2_01_every_envelope_declares_1_2_and_the_schema_tree_says_l2() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(canon_cli::json_out::DECLARED_MINOR, "1.2");
    assert_eq!(canon_cli::spec::LEVEL, "L2");
    let init = Command::new(CANON).args(["init", "--providers", "none"]).current_dir(dir.path()).env_remove("CANON_ROLE").output().expect("init");
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    for args in [&["status", "--json"][..], &["nosuch", "--json"], &["schema", "--json"], &["schema", "--errors", "--json"], &["schema", "--warnings", "--json"]] {
        let document = canon_in(dir.path(), args);
        assert_eq!(document["meta"]["contractVersion"], "1.2", "{args:?}: {document}");
        assert_eq!(atip::envelope_problems(&document), Vec::<String>::new(), "{args:?}");
    }
    let schema = canon_in(dir.path(), &["schema", "--json"]);
    assert_eq!((schema["data"]["kind"].as_str(), schema["data"]["level"].as_str(), schema["meta"]["schemaVersion"].as_str()), (Some("tree"), Some("L2"), Some("1.2")), "{schema}");
}

/// Whether `text` says that canon's `contractVersion` is `1.1`, in any of the ways the repository writes it (JSON, escaped JSON, code spans, prose).
fn spells_1_1(text: &str) -> bool {
    text.match_indices("contractVersion").any(|(at, word)| {
        let after: String = text[at + word.len()..].chars().take(12).filter(|c| !matches!(c, '"' | '\\' | ':' | '`' | '(' | ' ' | '=' | '\'')).collect();
        after.starts_with("1.1") && !after.starts_with("1.1.")
    })
}

#[test]
fn atip_claiml2_02_no_consumer_spells_canons_declaration_as_1_1() {
    assert!(spells_1_1("\"contractVersion\": \"1.1\"") && spells_1_1("`contractVersion` (`1.1`, on every envelope)") && spells_1_1("{\\\"contractVersion\\\":\\\"1.1\\\"}"), "the detector sees the spellings");
    assert!(!spells_1_1("\"contractVersion\": \"1.2\"") && !spells_1_1("`contractVersion` (`1.2`)") && !spells_1_1("contractVersion 1.9"));
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
                if spells_1_1(&text) {
                    found.push(path.strip_prefix(&root).unwrap_or(&path).display().to_string());
                }
            }
        }
    }
    found.sort();
    assert!(found.is_empty(), "these files spell canon's contractVersion as 1.1: {found:?}");
}

#[test]
fn atip_claiml2_03_a_canon_that_declares_l2_and_breaks_one_rule_of_l2_is_red_on_that_row() {
    // canon with one planted violation: `skills show` without a name is refused, the way 0.34 refused it (the name was required): `L2.skills` fails.
    let dir = tempfile::tempdir().expect("tempdir");
    let body = format!("if [ \"$1 $2 $3\" = \"skills show --json\" ]; then printf '%s\\n' '{{\"ok\":false,\"error\":{{\"code\":\"USAGE\",\"message\":\"the following required arguments were not provided: NAME\",\"retryable\":false,\"field\":\"NAME\"}},\"meta\":{{\"tool\":\"canon\",\"toolVersion\":\"0.35.0\",\"contract\":\"atip/1\",\"contractVersion\":\"1.2\",\"schemaVersion\":\"1.0\"}}}}'; exit 2; fi\nexec '{CANON}' \"$@\"\n");
    let bin = wrapper(dir.path(), "canon-with-a-required-guide-name", &body);
    let report = run(&bin, Level::L2, &fixture_path());
    assert!(!report.passed, "{}", report.text());
    assert_eq!(red_musts(&report), ["L2.skills"], "the violated row is the only one that fails: {}", report.text());
    let skills = report.find("L2.skills").expect("L2.skills");
    assert_eq!(skills.result, CheckResult::Fail);
    assert!(skills.reason.as_deref().is_some_and(|reason| !reason.is_empty()), "the row says why: {skills:?}");
}

#[test]
fn atip_claiml2_04_canon_passes_every_applicable_row_of_l2_and_the_rows_it_does_not_offer_are_unsupported() {
    let report = run(Path::new(CANON), Level::L2, &fixture_path());
    assert!(report.passed, "{}", report.text());
    assert_eq!((report.level, report.contract_version), ("L2", "1.2"), "{}", report.text());
    assert_eq!(red_musts(&report), Vec::<&str>::new(), "{}", report.text());
    for id in ["L2.stream", "L2.async", "L2.operation-key", "L2.search"] {
        assert_eq!(report.find(id).map(|check| check.result), Some(CheckResult::Unsupported), "{id}: {}", report.text());
    }
    for id in ["L1.tree", "L2.input-refusals", "L2.input-fragment", "L2.batch-preflight", "L2.batch-incomplete", "L2.idempotency", "L2.destructive-preview", "L2.destructive-commit", "L2.skills", "L2.guide-commands", "L2.help-guide", "L2.effects"] {
        assert_eq!(report.find(id).map(|check| check.result), Some(CheckResult::Pass), "{id}: {}", report.text());
    }
}

/// The committed fixture as a value, and a copy of it written under `dir` after `change`.
fn changed_fixture(dir: &Path, change: impl FnOnce(&mut Value)) -> PathBuf {
    let mut fixture: Value = serde_json::from_str(&read(&fixture_path())).expect("the fixture is JSON");
    change(&mut fixture);
    let path = dir.join("canon.fixture.json");
    std::fs::write(&path, serde_json::to_string_pretty(&fixture).expect("json")).expect("write the fixture");
    path
}

#[test]
fn atip_claiml2_05_a_fixture_without_the_previews_leaves_the_destructive_rows_unprovided() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = changed_fixture(dir.path(), |fixture| {
        for entry in fixture["commands"].as_object_mut().expect("commands").values_mut() {
            entry.as_object_mut().expect("an entry").remove("preview");
        }
    });
    let report = run(Path::new(CANON), Level::L2, &fixture);
    assert!(!report.passed, "{}", report.text());
    let destructive = ["L2.destructive-preview", "L2.destructive-preview-stdin", "L2.destructive-stale", "L2.destructive-half", "L2.destructive-commit"];
    assert_eq!(red_musts(&report), destructive, "{}", report.text());
    for id in destructive {
        let check = report.find(id).expect(id);
        assert_eq!(check.result, CheckResult::Unprovided, "{id}");
        assert!(check.reason.as_deref().unwrap_or_default().contains("no `preview`"), "{id}: {:?}", check.reason);
    }
}

#[test]
fn atip_claiml2_06_input_too_large_names_the_field_input() {
    let dir = tempfile::tempdir().expect("tempdir");
    let init = Command::new(CANON).args(["init", "--providers", "none"]).current_dir(dir.path()).env_remove("CANON_ROLE").output().expect("init");
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let big = dir.path().join("big.json");
    std::fs::write(&big, vec![b' '; 8 * 1024 * 1024 + 1]).expect("write");
    let bytes = canon_in(dir.path(), &["scenario", "new", "--input", big.to_str().expect("utf8"), "--json"]);
    let items = format!("[{}]", vec!["{}"; 10_001].join(","));
    let many = canon_in(dir.path(), &["scenario", "new", "--input", &items, "--json"]);
    for (document, reason) in [(&bytes, "TOO_MANY_BYTES"), (&many, "TOO_MANY_ITEMS")] {
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
fn atip_claiml2_07_just_atip_and_the_jobs_run_canon_at_l0_and_l2_and_the_checker_at_l0_at_1_2() {
    let root = repo_root();
    let workflow = read(&root.join(".github/workflows/test.yml"));
    let justfile = read(&root.join("justfile"));
    let canon_at = |level: &str| format!("target/debug/atip check --bin target/debug/canon --level {level} --contract 1.2 --fixture crates/canon-cli/tests/atip/canon.fixture.json");
    let checker = "target/debug/atip check --bin target/debug/atip --level L0 --contract 1.2 --fixture crates/atip-check/atip.fixture.json";
    let recipe = recipe_lines(&justfile, "atip");
    for line in [canon_at("L0"), canon_at("L2"), checker.to_string()] {
        assert!(recipe.contains(&line), "the recipe runs `{line}`: {recipe:?}");
    }
    assert!(recipe.iter().any(|line| line.contains("cargo build") && line.contains("-p canon-cli") && line.contains("-p atip-check")), "{recipe:?}");
    assert!(!recipe.iter().any(|line| line.contains("--level L1")), "the L2 report holds the L1 rows: {recipe:?}");
    let l0 = job_commands(&workflow, "atip-l0");
    assert!(l0.contains(&canon_at("L0")) && l0.contains(&checker.to_string()), "the job atip-l0 runs canon and the checker at L0: {l0:?}");
    let l2 = job_commands(&workflow, "atip-l2");
    assert!(l2.contains(&canon_at("L2")), "the job atip-l2 runs canon at L2: {l2:?}");
    assert!(l2.iter().any(|line| line.contains("cargo build") && line.contains("-p canon-cli") && line.contains("-p atip-check")), "{l2:?}");
    assert!(!workflow.lines().any(|line| line == "  atip-l1:"), "the 0.34 job atip-l1 is folded into atip-l2");
    let ci = justfile.lines().find(|line| line.starts_with("ci:")).expect("the recipe ci");
    assert!(ci.split_whitespace().any(|word| word == "atip"), "just ci runs the recipe atip: {ci}");
}

#[test]
fn atip_claiml2_08_a_canon_whose_plain_answer_says_1_1_fails_l0_contract_version_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let body = format!("out=$('{CANON}' \"$@\")\ncode=$?\nif [ -n \"$out\" ]; then printf '%s\\n' \"$out\" | sed 's/\"contractVersion\":\"1.2\"/\"contractVersion\":\"1.1\"/g'; fi\nexit $code\n");
    let bin = wrapper(dir.path(), "canon-that-says-1-1", &body);
    let report = run(&bin, Level::L0, &fixture_path());
    assert!(!report.passed, "{}", report.text());
    assert_eq!(red_musts(&report), ["L0.contract-version"], "{}", report.text());
}

#[test]
fn atip_claiml2_09_the_fixture_claims_1_2_and_lists_what_the_l2_rows_need() {
    let fixture: Value = serde_json::from_str(&read(&fixture_path())).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], "1.2");
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

#[test]
fn atip_claiml2_10_a_fixture_that_runs_dashboard_outside_not_run_does_not_pass_a_level() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fixture = changed_fixture(dir.path(), |fixture| {
        fixture.as_object_mut().expect("object").remove("notRun");
        fixture["timeoutSeconds"] = json!(4);
    });
    // From L1 the checker knows `dashboard` serves (its `effectNote`) and runs it with the rest: it is signalled by the time limit, which no row passes.
    let args = CheckArgs { bin: CANON.into(), level: Level::L1, fixture: fixture.into(), contract: Some("1.2".to_string()) };
    match check_binary(&args) {
        Err(failure) => assert!(!failure.message.is_empty()),
        Ok(report) => {
            assert!(!report.passed, "dashboard serves until it is signalled: {}", report.text());
            assert!(!red_musts(&report).is_empty(), "{}", report.text());
        }
    }
}

/// The part of the CHANGELOG that is the release being prepared (the section `## Unreleased`).
fn release_section(changelog: &str) -> &str {
    let start = changelog.find("\n## Unreleased").expect("the CHANGELOG has the release section");
    let rest = &changelog[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |at| at + 3);
    &rest[..end]
}

/// The obligations of contract 17.2 canon's tests stand for, and the one word each row of the evidence list is found by.
const OBLIGATIONS: [&str; 8] = ["tree complete", "exit codes registered", "write constraints", "preview equals commit", "secrets", "examples and guides parse", "next syntax", "warnings and stopped midway"];

/// What is wrong with the evidence list of `section`: an obligation without a row or without a test, a test that is not a `fn` of the sources.
fn evidence_problems(section: &str, crates: &Path) -> Vec<String> {
    let Some(start) = section.find("**Evidence of contract 17.2**") else { return vec!["the section has no evidence list".to_string()] };
    let table: Vec<&str> = section[start..].lines().skip_while(|line| !line.starts_with('|')).take_while(|line| line.starts_with('|')).collect();
    let mut problems = Vec::new();
    for obligation in OBLIGATIONS {
        let Some(row) = table.iter().find(|line| line.starts_with(&format!("| {obligation} |"))) else {
            problems.push(format!("no row for `{obligation}`"));
            continue;
        };
        let tests: Vec<&str> = row.split('`').skip(1).step_by(2).collect();
        if tests.is_empty() {
            problems.push(format!("`{obligation}` names no test"));
        }
        for test in tests {
            let Some((file, name)) = test.split_once("::") else {
                problems.push(format!("`{test}` is not `path::fn`"));
                continue;
            };
            match std::fs::read_to_string(crates.join(file)) {
                Ok(source) if source.contains(&format!("fn {name}(")) => {}
                Ok(_) => problems.push(format!("`{obligation}`: {file} has no test `{name}`")),
                Err(_) => problems.push(format!("`{obligation}`: {file} does not exist")),
            }
        }
    }
    problems
}

#[test]
fn atip_claiml2_11_every_obligation_of_contract_17_2_has_a_row_with_tests_that_exist() {
    let root = repo_root();
    let changelog = read(&root.join("CHANGELOG.md"));
    assert_eq!(evidence_problems(release_section(&changelog), &root.join("crates")), Vec::<String>::new());
}

#[test]
fn atip_claiml2_12_a_row_taken_out_of_the_list_or_a_test_that_does_not_exist_fails_the_check() {
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
}

/// The entries the 0.35.0 section carries for the breaking changes of the release: a name, and a spelling the entry has.
const RELEASE_ENTRIES: [(&str, &str); 14] = [
    ("the declaration 1.2 and level L2", "claims level L2"),
    ("a cursor of 0.34 is refused", "`FOREIGN`"),
    ("limit, cursor, all and fields are flag-only", "flag-only"),
    ("the plan digest is sha256: and the JCS of effects and inputs", "`sha256:`"),
    ("PLAN_STALE and the preview before a move", "`PLAN_STALE`"),
    ("a # line of an NDJSON file", "NDJSON"),
    ("INPUT_INVALID with INVALID_VALUE for a value outside a set", "`INVALID_VALUE`"),
    ("+1 on argv", "`+1`"),
    ("the pointers of a batch", "`BATCH_INCOMPLETE`"),
    ("the --dry-run of the preflight is gone", "`--dry-run`"),
    ("the JSON shape of skills show", "`data.text`"),
    ("the first line of the help", "\"canon skills show\" prints the guide"),
    ("the codes that moved to STOPPED_MIDWAY", "`STOPPED_MIDWAY`"),
    ("the checker declares 1.2 and runs L2", "`atip check --level L2"),
];

fn release_problems(section: &str) -> Vec<String> {
    RELEASE_ENTRIES.iter().filter(|(_, spelling)| !section.contains(spelling)).map(|(name, _)| format!("the release section has no entry for: {name}")).collect()
}

#[test]
fn atip_claiml2_13_the_release_section_carries_every_breaking_change_and_the_evidence_list() {
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
    for (name, spelling) in RELEASE_ENTRIES {
        let problems = release_problems(&section.replace(spelling, "(taken out)"));
        assert!(problems.iter().any(|problem| problem.ends_with(name)), "{name}: {problems:?}");
    }
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
    let first = canon_in(dir.path(), &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "fixed", "--json"]);
    assert_eq!(first["ok"], true, "{first}");
    for count in [1, 2] {
        let batch = Value::Array(vec![close.clone(); count]).to_string();
        let document = canon_in(dir.path(), &["finding", "close", "--input", &batch, "--json"]);
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
    let preview = canon_in(dir.path(), &["artifacts", "move", "--tier", "local", "--json"]);
    assert_eq!((preview["ok"].clone(), preview["data"]["committed"].clone()), (json!(true), json!(false)), "{preview}");
    let digest = preview["data"]["planDigest"].as_str().expect("a digest");
    let argv: Vec<&str> = preview["meta"]["next"][0]["argv"].as_array().expect("meta.next[0].argv").iter().map(|word| word.as_str().expect("a word")).collect();
    assert_eq!(argv.first(), Some(&"artifacts"), "{preview}");
    assert!(argv.contains(&"--commit") && argv.contains(&"--json") && argv.contains(&format!("--plan-digest={digest}").as_str()), "{argv:?}");
    let commit = canon_in(dir.path(), &argv);
    assert_eq!((commit["ok"].clone(), commit["data"]["committed"].clone()), (json!(true), json!(true)), "{commit}");
}
