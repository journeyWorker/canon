//! `canon-0-39-session-ingest-removal`: `canon ingest sessions` is gone and the rest of `ingest` is as it was.
//!
//! F1 the command is an unknown subcommand and the selftest suite a closed-value refusal, F2 the plan cursor file name and `regime_key` hash
//! (`canon_ingest::digest::content_digest`, pinned in its own module) are unchanged, F3 a dispatched Run is written once and a repeat is not an
//! error, F4 an old `ingest:` section is ignored, F5 the skills bundle no longer carries the guide and an old install's copy is an orphan, F6 the
//! retired scenarios leave with their `.feature` file, F7 the guides say nothing of the command.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;

mod support;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn canon_in(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(dir).env("CANON_ACTOR", "canon-test").output().expect("spawn canon")
}

fn json_of(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let out = canon_in(dir.path(), &["init", "--repo", ".", "--no-harness"]);
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    dir
}

/// ingest.removal.01 (F1): the command is refused and the help names what is left.
/// F1: whatever options follow, `ingest sessions` is refused by the generic unknown-command path, naming the two subcommands `ingest` has.
#[test]
fn ingest_removal_01_ingest_sessions_is_an_unknown_subcommand_of_ingest() {
    let dir = tempfile::tempdir().unwrap();
    for args in [&["ingest", "sessions", "--json"][..], &["ingest", "sessions", "--watch", "--json"], &["ingest", "sessions", "--home", ".", "--all-workspaces", "--json"]] {
        let out = canon_in(dir.path(), args);
        assert_eq!(out.status.code(), Some(2), "canon {args:?}: {}", String::from_utf8_lossy(&out.stdout));
        let error = &json_of(&out)["error"];
        assert_eq!(error["code"], "UNKNOWN_COMMAND", "canon {args:?}: {error}");
        assert_eq!(error["received"], "sessions", "canon {args:?}: {error}");
        let mut expected: Vec<&str> = error["expected"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect();
        expected.sort_unstable();
        assert_eq!(expected, ["plans", "records"], "canon {args:?}: {error}");
    }
}

/// F1: the `ingest` help shows two subcommands and examples of those two only.
#[test]
fn ingest_removal_01_the_ingest_help_names_plans_and_records_only() {
    let dir = tempfile::tempdir().unwrap();
    let out = canon_in(dir.path(), &["ingest", "--help"]);
    let help = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(help.contains("canon ingest records") && help.contains("canon ingest plans"), "{help}");
    assert!(!help.contains("sessions"), "{help}");
}

/// ingest.removal.02 (F1, failure path): the selftest suite and the generated documents keep no trace of the command.
/// F1: `--suite session-ingest` is outside the closed set (exit 2); a run with no `--suite` runs the suites that are left.
#[test]
fn ingest_removal_02_selftest_suite_session_ingest_is_a_closed_value_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let out = canon_in(dir.path(), &["selftest", "--suite", "session-ingest", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stdout));
    let error = &json_of(&out)["error"];
    assert_eq!(error["code"], "USAGE", "{error}");
    assert!(!error["expected"].as_array().unwrap().iter().any(|name| name == "session-ingest"), "{error}");
    let all = canon_in(dir.path(), &["selftest", "--json"]);
    assert_eq!(all.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&all.stdout), String::from_utf8_lossy(&all.stderr));
}

/// F1: the generated command document, the schema and the website reference say nothing of the command (the drift test `atip_l0` holds the document equal to the generator).
#[test]
fn ingest_removal_02_the_generated_documents_have_no_ingest_sessions_line() {
    let root = repo_root();
    for path in ["docs/cli/canon.cli.json", "packages/website/src/content/docs/cli.mdx", "packages/website/src/content/docs/ko/cli.mdx", "canon/skills/reference/role-orchestrator.md"] {
        let text = fs::read_to_string(root.join(path)).unwrap();
        assert!(!text.contains("ingest sessions") && !text.contains("session-ingest"), "{path} still names the removed command");
    }
    let dir = tempfile::tempdir().unwrap();
    let schema = String::from_utf8_lossy(&canon_in(dir.path(), &["schema", "--json"]).stdout).into_owned();
    assert!(!schema.contains("ingest sessions") && !schema.contains("IngestSessionsReport"), "canon schema still names the removed command");
}

/// ingest.removal.12 (F7): the guides say nothing of the command.
/// F7: the guides a person reads do not name the command, and the two env vars are not in the spec.
#[test]
fn ingest_removal_12_the_readme_architecture_and_website_do_not_name_the_removed_command() {
    let root = repo_root();
    let mut files = vec![root.join("README.md"), root.join("ARCHITECTURE.md")];
    let mut dirs = vec![root.join("packages/website/src")];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "mdx" || e == "ts" || e == "astro") {
                files.push(path);
            }
        }
    }
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("ingest sessions") && !text.contains("canon-session-ingest"), "{} names the removed command", path.display());
    }
    assert!(!root.join("packages/website/src/content/docs/privacy.mdx").exists() && !root.join("packages/website/src/content/docs/ko/privacy.mdx").exists());
    let spec = fs::read_to_string(root.join("docs/cli/canon.cli.json")).unwrap();
    assert!(!spec.contains("CANON_INGEST_CLAUDE_SESSIONS_DIR") && !spec.contains("CANON_INGEST_OMP_SESSIONS_DIR"));
}

/// ingest.removal.13 (F7, failure path): the release check fails when the CHANGELOG entry is missing.
/// F7: the CHANGELOG's Breaking table names the command, both variables, both keys, the suite and the skill; and says stored records are left alone.
#[test]
fn ingest_removal_13_the_changelog_lists_what_was_removed() {
    let changelog = fs::read_to_string(repo_root().join("CHANGELOG.md")).unwrap();
    let unreleased = changelog.split("\n## ").find(|section| section.starts_with("0.39")).unwrap_or_default();
    for needed in [
        "`canon ingest sessions`",
        "`CANON_INGEST_CLAUDE_SESSIONS_DIR`",
        "`CANON_INGEST_OMP_SESSIONS_DIR`",
        "`ingest.sources.<id>.roots`",
        "`ingest.sessions.privacy`",
        "`selftest --suite session-ingest`",
        "`canon-session-ingest`",
        "`.canon/ingest/cursors/omp.json`",
        "UNKNOWN_COMMAND",
    ] {
        assert!(unreleased.contains(needed), "the 0.39 section of CHANGELOG.md does not mention {needed}");
    }
}

/// ingest.removal.08 (F5): the bundle no longer carries the guide.
/// F5: the bundle names no `canon-session-ingest` anywhere, and `skills show` has no such guide.
#[test]
fn ingest_removal_08_the_skills_bundle_no_longer_carries_the_session_ingest_guide() {
    let root = repo_root();
    assert!(!root.join("canon/skills/reference/canon-session-ingest.md").exists());
    for path in ["canon/skills/SKILL.src.md", "canon/knowledge-index.json", "canon/skills/reference/canon-artifact-ingest.md"] {
        let text = fs::read_to_string(root.join(path)).unwrap();
        assert!(!text.contains("canon-session-ingest") && !text.contains("session-reference"), "{path} still names the guide");
    }
    let dir = tempfile::tempdir().unwrap();
    let out = canon_in(dir.path(), &["skills", "show", "canon-session-ingest"]);
    assert_ne!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
}

/// ingest.removal.09 (F5): an old install's guide is an orphan.
/// F5 failure path: an old install's copy of the guide is an orphan (`skills check` names it, exit 1); `skills install` removes it while its bytes match the lock, and the check then passes.
#[test]
fn ingest_removal_09_an_old_installs_session_ingest_guide_is_an_orphan_that_install_removes() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = (repo_root().join("canon/skills"), tmp.path().join("target"));
    fs::create_dir_all(&target).unwrap();
    let run = |args: &[&str]| hermetic::canon().args(args).arg("--skills-source").arg(&source).arg("--repo").arg(&target).output().unwrap();
    let installed = run(&["skills", "install", "--harness", "claude"]);
    assert_eq!(installed.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&installed.stdout), String::from_utf8_lossy(&installed.stderr));
    assert_eq!(run(&["skills", "check", "--harness", "claude"]).status.code(), Some(0), "the new bundle passes");
    assert_eq!(run(&["skills", "doctor", "--harness", "claude"]).status.code(), Some(0));

    let lock_path = target.join(".canon/skills/.install-lock.json");
    let mut lock: Value = serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    let key = ".claude/skills/canon/reference/canon-session-ingest.md";
    let old = "# canon-session-ingest\n";
    fs::create_dir_all(target.join(key).parent().unwrap()).unwrap();
    fs::write(target.join(key), old).unwrap();
    lock["files"][key] = canon_cli::skills::content_hash(old.as_bytes()).into();
    fs::write(&lock_path, serde_json::to_string_pretty(&lock).unwrap()).unwrap();

    let check = run(&["skills", "check", "--harness", "claude"]);
    assert_eq!(check.status.code(), Some(1), "{}", String::from_utf8_lossy(&check.stdout));
    let said = format!("{}{}", String::from_utf8_lossy(&check.stdout), String::from_utf8_lossy(&check.stderr));
    assert!(said.contains("canon-session-ingest"), "the orphan is named: {said}");

    let reinstall = run(&["skills", "install", "--harness", "claude"]);
    assert_eq!(reinstall.status.code(), Some(0), "{}", String::from_utf8_lossy(&reinstall.stderr));
    assert!(!target.join(key).exists(), "an unmodified orphan is removed");
    assert_eq!(run(&["skills", "check", "--harness", "claude"]).status.code(), Some(0));
}

/// ingest.removal.06 (F4): a canon.yaml still holding the old `ingest:` section gives the answers it gave without it.
#[test]
fn ingest_removal_06_an_old_ingest_section_in_canon_yaml_is_not_read() {
    let plain = init_repo();
    let old = init_repo();
    let yaml_path = old.path().join("canon.yaml");
    let mut yaml = fs::read_to_string(&yaml_path).unwrap();
    yaml.push_str("ingest:\n  sources:\n    omp:\n      roots: [\"/nowhere\"]\n  sessions:\n    privacy:\n      capture_user_directives: true\n      max_directive_chars: -5\n");
    fs::write(&yaml_path, &yaml).unwrap();

    for args in [&["status", "--json"][..], &["gate", "check", "--json"]] {
        let (a, b) = (canon_in(plain.path(), args), canon_in(old.path(), args));
        assert_eq!(a.status.code(), b.status.code(), "canon {args:?}");
        let strip = |out: &Output| {
            let mut doc = json_of(out);
            if let Some(data) = doc["data"].as_object_mut() {
                data.retain(|key, _| key != "generatedAt" && key != "at");
            }
            doc["data"].clone()
        };
        assert_eq!(strip(&a), strip(&b), "canon {args:?}: the old section changes nothing");
        assert!(json_of(&b)["warnings"].as_array().is_none_or(Vec::is_empty), "canon {args:?}: no warning for the old section: {}", json_of(&b));
    }
}

/// ingest.removal.07 (F4, failure path): a broken canon.yaml is still the command's config error, whatever sections it holds.
#[test]
fn ingest_removal_07_a_broken_canon_yaml_is_still_a_config_error() {
    let old = init_repo();
    let yaml_path = old.path().join("canon.yaml");
    fs::write(&yaml_path, "ingest:\n  sources: {}\ntiers: [unclosed\n").unwrap();
    let broken = canon_in(old.path(), &["gate", "check", "--json"]);
    assert_ne!(broken.status.code(), Some(0));
    assert!(json_of(&broken)["error"]["code"].as_str().unwrap().starts_with("CONFIG_"), "{}", json_of(&broken));
}

/// ingest.removal.04 (F3): `dispatch begin` then `end` write the Run to the routed tier (the repeat and other-content cases are the unit test `ingest_removal_05_…`).
#[test]
fn ingest_removal_04_dispatch_begin_and_end_write_the_run_to_the_routed_tier() {
    let dir = init_repo();
    let begin = canon_in(dir.path(), &["dispatch", "begin", "--role", "implementer", "--regime", "implementer/app/api/abcdef012345", "--actor", "a", "--json"]);
    assert_eq!(begin.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&begin.stdout), String::from_utf8_lossy(&begin.stderr));
    let run_id = json_of(&begin)["data"]["runId"].as_str().expect("begin names the run").to_string();
    let query = |dir: &Path| json_of(&canon_in(dir, &["query", "--kind", "run", "--json"]))["data"]["records"].as_array().unwrap().len();
    assert_eq!(query(dir.path()), 1, "begin persists the Run to the hot rung");
    let end = canon_in(dir.path(), &["dispatch", "end", &run_id, "--run-outcome", "succeeded", "--json"]);
    assert_eq!(end.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&end.stdout), String::from_utf8_lossy(&end.stderr));
}

/// ingest.removal.10 (F6): no `ingest.sessions.*` scenario remains in the corpus; the hot-store-location scenario moved to `plans.feature` as `ingest.plans.14` with its flow tag and body.
#[test]
fn ingest_removal_10_the_session_scenarios_left_with_their_feature_file_and_the_hot_store_one_moved() {
    let ingest = repo_root().join(".canon/specs/kind=feature/area=ingest");
    assert!(!ingest.join("sessions.feature").exists());
    let plans = fs::read_to_string(ingest.join("plans.feature")).unwrap();
    assert!(plans.contains("@ingest.plans.14"));
    assert!(plans.contains("@flow:canon-0-30-record-links.F5\n  @ingest.plans.14\n  Scenario: The other commands that write hot records print the hot store location too"));
    assert!(!plans.contains("ingest.sessions."));
    let mut dirs = vec![repo_root().join(".canon/specs")];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "feature") {
                assert!(!fs::read_to_string(&path).unwrap().contains("ingest.sessions."), "{} still carries an ingest.sessions scenario", path.display());
            }
        }
    }
}

/// ingest.removal.11 (F6, failure path): a scenario whose `.feature` file is deleted gets a tombstone on the next sync and leaves the corpus; it is not an uncovered cell.
#[test]
fn ingest_removal_11_a_scenario_whose_feature_file_is_deleted_is_tombstoned_and_leaves_query() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  scenario: local\n").unwrap();
    support::declare_scenario(dir.path(), "demo", "gone.leaf.01", None);
    support::declare_scenario(dir.path(), "demo", "kept.leaf.01", None);
    let scenarios = |dir: &Path| -> Vec<String> {
        let out = canon_in(dir, &["query", "--kind", "scenario", "--all", "--json"]);
        json_of(&out)["data"]["records"].as_array().unwrap().iter().map(|record| record["scenario_id"].as_str().unwrap_or_default().to_string()).collect()
    };
    assert_eq!(canon_in(dir.path(), &["inventory", "sync"]).status.code(), Some(0));
    assert!(scenarios(dir.path()).contains(&"gone.leaf.01".to_string()));
    fs::remove_file(dir.path().join("specs-demo/kind=feature/area=gone/leaf.feature")).unwrap();
    assert_eq!(canon_in(dir.path(), &["inventory", "sync"]).status.code(), Some(0));
    let left = scenarios(dir.path());
    assert!(!left.contains(&"gone.leaf.01".to_string()) && left.contains(&"kept.leaf.01".to_string()), "{left:?}");
}

fn git_in(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn commit_all(dir: &Path, message: &str) {
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", message]);
}

/// The `stale-evidence` subjects of `gate check` in `dir`, sorted, and the `staleness_unresolvable` advisory when it says one.
fn stale_subjects(dir: &Path) -> (Vec<String>, Option<String>) {
    let body = json_of(&canon_in(dir, &["gate", "check", "--json"]));
    let mut subjects: Vec<String> =
        body["data"]["violations"].as_array().into_iter().flatten().filter(|v| v["class"] == "stale-evidence").map(|v| v["subject"].as_str().unwrap().to_string()).collect();
    subjects.sort();
    let advisory = body["data"]["advisories"].as_array().into_iter().flatten().find(|a| a["kind"] == "staleness_unresolvable").map(|a| a["message"].as_str().unwrap().to_string());
    (subjects, advisory)
}

/// ingest.removal.16 (F6, failure path): the green evidence of a scenario whose `.feature` file was deleted and tombstoned is not `stale-evidence` however many commits have passed since, and the cell is not counted as one whose base commit git could not read; the evidence of the scenario that is still declared is both.
#[test]
fn ingest_removal_16_the_old_evidence_of_a_tombstoned_scenario_is_not_stale_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git_in(repo, &["init", "-q", "-b", "main"]);
    assert!(canon_in(repo, &["init", "--repo", ".", "--no-policy", "--json"]).status.success());
    fs::write(repo.join(".canon/policy.yaml"), "staleness:\n  max_commits_behind: 2\n").unwrap();
    for id in ["gone.leaf.01", "kept.leaf.01"] {
        assert_eq!(canon_in(repo, &["scenario", "new", id, "--title", id, "--case", "failure", "--json"]).status.code(), Some(0));
    }
    assert_eq!(canon_in(repo, &["inventory", "sync", "--json"]).status.code(), Some(0));
    fs::write(repo.join("code.rs"), "1").unwrap();
    commit_all(repo, "project");
    for id in ["gone.leaf.01", "kept.leaf.01"] {
        let out = canon_in(repo, &["evidence", "add", "--scenario", id, "--project", "root", "--class", "test-run", "--ref", "run", "--role", "implementer", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    }
    commit_all(repo, "evidence");
    for n in 2..=4 {
        fs::write(repo.join("code.rs"), n.to_string()).unwrap();
        commit_all(repo, &format!("code {n}"));
    }
    assert_eq!(stale_subjects(repo).0, ["gone.leaf.01", "kept.leaf.01"], "both are past the ceiling while both are declared");

    fs::remove_file(repo.join(".canon/specs/kind=feature/area=gone/leaf.feature")).unwrap();
    assert_eq!(canon_in(repo, &["inventory", "sync", "--json"]).status.code(), Some(0));
    commit_all(repo, "the gone feature file is deleted");
    assert_eq!(stale_subjects(repo).0, ["kept.leaf.01"], "the tombstoned scenario is absent for the staleness check, the declared one is not");

    let shallow = tempfile::tempdir().unwrap();
    let url = format!("file://{}", repo.display());
    git_in(shallow.path(), &["clone", "-q", "--depth", "1", &url, "."]);
    // A clone has no scenario index; `gate check` builds it from the feature files, where the deleted one is not.
    let (stale, advisory) = stale_subjects(shallow.path());
    assert!(stale.is_empty(), "{stale:?}");
    let advisory = advisory.expect("a shallow clone has no base commit for the declared scenario");
    assert!(advisory.starts_with("1 (subject, role) cell(s)") && advisory.contains("kept.leaf.01") && !advisory.contains("gone.leaf.01"), "{advisory}");
}

/// One scenario `gone.leaf.01` with green evidence, past `max_commits_behind = 2` (three commits after the evidence), its feature file the only one.
fn one_scenario_past_the_ceiling(repo: &Path) {
    git_in(repo, &["init", "-q", "-b", "main"]);
    assert!(canon_in(repo, &["init", "--repo", ".", "--no-policy", "--json"]).status.success());
    fs::write(repo.join(".canon/policy.yaml"), "staleness:\n  max_commits_behind: 2\n").unwrap();
    assert_eq!(canon_in(repo, &["scenario", "new", "gone.leaf.01", "--title", "gone", "--case", "failure", "--json"]).status.code(), Some(0));
    assert_eq!(canon_in(repo, &["inventory", "sync", "--json"]).status.code(), Some(0));
    fs::write(repo.join("code.rs"), "1").unwrap();
    commit_all(repo, "project");
    let out = canon_in(repo, &["evidence", "add", "--scenario", "gone.leaf.01", "--project", "root", "--class", "test-run", "--ref", "run", "--role", "implementer", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    commit_all(repo, "evidence");
    for n in 2..=4 {
        fs::write(repo.join("code.rs"), n.to_string()).unwrap();
        commit_all(repo, &format!("code {n}"));
    }
}

/// gate.tombstone-corpus.01 (F1): deleting the last feature file leaves an empty scenario list, and the tombstone, not the empty list, says the scenario is gone.
#[test]
fn gate_tombstone_corpus_01_the_old_evidence_of_the_last_deleted_scenario_is_not_stale_evidence_and_not_unresolvable() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    one_scenario_past_the_ceiling(repo);
    assert_eq!(stale_subjects(repo).0, ["gone.leaf.01"], "declared and past the ceiling");

    let feature = repo.join(".canon/specs/kind=feature/area=gone/leaf.feature");
    let text = fs::read_to_string(&feature).unwrap();
    fs::remove_file(&feature).unwrap();
    assert_eq!(canon_in(repo, &["inventory", "sync", "--json"]).status.code(), Some(0));
    commit_all(repo, "the last feature file is deleted");
    let scenarios = json_of(&canon_in(repo, &["query", "--kind", "scenario", "--all", "--json"]));
    assert!(scenarios["data"]["records"].as_array().unwrap().iter().all(|r| r["removed"] == true), "only tombstones are left: {scenarios}");
    assert_eq!(stale_subjects(repo).0, Vec::<String>::new(), "a tombstone is read as a fact, not guessed from an empty list");

    let shallow = tempfile::tempdir().unwrap();
    git_in(shallow.path(), &["clone", "-q", "--depth", "1", &format!("file://{}", repo.display()), "."]);
    // A clone does not carry the scenario index (and so no tombstone), so the repo's own is copied in: the clone then reads what a synced work tree reads, with a base commit it does not hold.
    let (_, advisory) = stale_subjects(shallow.path());
    assert!(advisory.is_some_and(|a| a.contains("gone.leaf.01")), "without the tombstone the clone cannot tell the scenario was deleted, and counts its cell");
    let index = scenario_index_dir(repo);
    copy_dir(&index, &shallow.path().join(index.strip_prefix(repo).unwrap()));
    let (stale, advisory) = stale_subjects(shallow.path());
    assert!(stale.is_empty() && advisory.is_none(), "{stale:?} {advisory:?}");
    let status = json_of(&canon_in(shallow.path(), &["status", "--json"]));
    assert!(!status.to_string().contains("staleness_unresolvable"), "{status}");

    fs::create_dir_all(feature.parent().unwrap()).unwrap();
    fs::write(&feature, text).unwrap();
    assert_eq!(canon_in(repo, &["inventory", "sync", "--json"]).status.code(), Some(0));
    commit_all(repo, "the feature file is back");
    assert_eq!(stale_subjects(repo).0, ["gone.leaf.01"], "a re-declared scenario is judged again");
}

/// gate.tombstone-corpus.02 (F1, failure path): a repository whose scenarios cannot be read keeps judging the evidence.
#[test]
fn gate_tombstone_corpus_02_a_corpus_that_is_not_read_still_judges_the_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    one_scenario_past_the_ceiling(repo);
    let yaml = fs::read_to_string(repo.join("canon.yaml")).unwrap();
    assert!(yaml.contains("  scenario:") || yaml.contains("routing:"), "{yaml}");
    // The scenario kind goes to another tier than the git one the gate reads.
    let routed = if yaml.contains("  scenario:") { yaml.replacen("  scenario: local", "  scenario: hot", 1) } else { yaml.replacen("routing:\n", "routing:\n  scenario: hot\n", 1) };
    fs::write(repo.join("canon.yaml"), routed).unwrap();
    fs::remove_file(repo.join(".canon/specs/kind=feature/area=gone/leaf.feature")).unwrap();
    commit_all(repo, "the scenario kind is routed off the git tier and the feature file is gone");
    assert_eq!(stale_subjects(repo).0, ["gone.leaf.01"], "no corpus was read, so nothing is known to be absent");
}

/// The directory of the scenario index (`kind=scenario`) under `repo`'s `.canon`.
fn scenario_index_dir(repo: &Path) -> PathBuf {
    fn find(dir: &Path) -> Option<PathBuf> {
        for entry in fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "kind=scenario") {
                    return Some(path);
                }
                if let Some(found) = find(&path) {
                    return Some(found);
                }
            }
        }
        None
    }
    find(&repo.join(".canon")).expect("a synced repo has a scenario index")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
