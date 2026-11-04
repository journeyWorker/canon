//! Refusals and read commands write only what they declare (`canon-0-35-read-writes`, scenarios `atip.readwrites.*`),
//! against the built binary: a refused call leaves the repository, `HOME` and the temporary directory byte for byte as
//! they were (spec 8.2, 12.2), even when the scenario index is stale and the call is one that refreshes it; a call that
//! is accepted refreshes the index as its `effectNote` says; `--dry-run`, `--help`, `--version`, `schema` and `skills
//! show` write nothing; `report check` and `selftest` leave no file in the temporary directory; `dashboard` is declared
//! a server.
//!
//! Tests are named by scenario id (`atip_readwrites_01_…` proves `atip.readwrites.01`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

/// One run's world: the repository, a private `HOME` and a private temporary directory.
struct World {
    repo: TempDir,
    home: TempDir,
    tmp: TempDir,
}

impl World {
    fn repo(&self) -> &Path {
        self.repo.path()
    }

    fn canon(&self, args: &[&str]) -> Output {
        self.canon_in(self.repo(), args)
    }

    fn canon_in(&self, dir: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_canon"))
            .args(args)
            .current_dir(dir)
            .env_remove("CANON_ROLE")
            .env("CANON_ACTOR", "atip-readwrites")
            .env("HOME", self.home.path())
            .env("TMPDIR", self.tmp.path())
            .env_remove("CODEX_HOME")
            .env_remove("HERMES_HOME")
            .env_remove("CANON_INGEST_CLAUDE_SESSIONS_DIR")
            .env_remove("CANON_INGEST_OMP_SESSIONS_DIR")
            .stdin(Stdio::null())
            .output()
            .expect("spawn canon")
    }

    fn ok(&self, args: &[&str]) {
        let out = self.canon(args);
        assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }

    /// Every file of the repository (but `.git` and the hot tier's SQLite files), `HOME` and the temporary directory, with its bytes.
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, label: &str, out: &mut BTreeMap<String, Vec<u8>>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                if rel == ".git" || rel.starts_with(".canon/hot.db") {
                    continue;
                }
                if path.is_dir() {
                    out.insert(format!("{label}/{rel}/"), Vec::new());
                    walk(root, &path, label, out);
                } else {
                    out.insert(format!("{label}/{rel}"), std::fs::read(&path).unwrap_or_default());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(self.repo(), self.repo(), "repo", &mut out);
        walk(self.home.path(), self.home.path(), "home", &mut out);
        walk(self.tmp.path(), self.tmp.path(), "tmp", &mut out);
        out
    }

    fn scenario_index(&self) -> PathBuf {
        self.repo().join(".canon/ledger/kind=scenario")
    }

    fn indexed(&self) -> bool {
        std::fs::read_dir(self.scenario_index()).is_ok_and(|mut entries| entries.next().is_some())
    }
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn code_of(output: &Output) -> (Option<i32>, String) {
    let body = json_of(output);
    (output.status.code(), body["error"]["code"].as_str().unwrap_or_default().to_string())
}

const FEATURE: &str = ".canon/specs/kind=feature/area=auth/login.feature";

/// A git repo with subject `demo`, change `add-login`, and scenarios `auth.login.01` and `.02` that only the feature file declares: the scenario
/// index is stale (no record yet), which is what every writer refreshes.
fn world() -> World {
    let world = World { repo: TempDir::new().unwrap(), home: TempDir::new().unwrap(), tmp: TempDir::new().unwrap() };
    let repo = world.repo();
    let git = |args: &[&str]| {
        let out = Command::new("git").current_dir(repo).args(["-c", "user.name=canon", "-c", "user.email=canon@example.invalid"]).args(args).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q"]);
    world.ok(&["init", "--providers", "none"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "fixture"]);
    world.ok(&["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    world.ok(&["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    world.ok(&["feature", "new", "auth.login", "--title", "Login"]);
    world.ok(&["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", FEATURE]);
    world.ok(&["scenario", "new", "auth.login.02", "--title", "Sign out", "--feature", FEATURE]);
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    std::fs::write(repo.join("reports/match.xml"), r#"<testsuite><testcase name="auth.login.01 signs in"/><testcase name="auth.login.02 signs out"/></testsuite>"#).unwrap();
    std::fs::write(repo.join("reports/nomatch.xml"), r#"<testsuite><testcase name="nosuch.x.01 nothing"/></testsuite>"#).unwrap();
    assert!(!world.indexed(), "the fixture's scenario index is stale");
    world
}

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The calls that refresh the index, each refused by something no index is needed to know (a missing object, a transition, a bad value),
/// and by something only the (new) index can tell (a scenario no feature file declares).
fn refused_calls() -> Vec<(&'static str, Vec<&'static str>, &'static str)> {
    vec![
        ("subject show", vec!["subject", "show", "nosuch", "--json"], "SUBJECT_NOT_FOUND"),
        ("subject move: no such subject", vec!["subject", "move", "nosuch", "--to", "specced", "--json"], "SUBJECT_NOT_FOUND"),
        ("subject move: not a transition", vec!["subject", "move", "demo", "--to", "shipped", "--json"], "SUBJECT_CONFLICT"),
        ("subject move: no reason", vec!["subject", "move", "demo", "--to", "specced", "--reason", "two\nlines", "--json"], "SUBJECT_ARGUMENT_INVALID"),
        ("evidence add --all-scenarios: no case names a scenario", vec!["evidence", "add", "--project", "root", "--all-scenarios", "--class", "test-run", "--ref", "x", "--role", "implementer", "--report", "junit:reports/nomatch.xml", "--json"], "EVIDENCE_BIND_REFUSED"),
        ("evidence add: a revision that is not one", vec!["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "x", "--role", "implementer", "--verdict", "faithful", "--rev", "no-such-revision", "--json"], ""),
        ("evidence audit: bad digest", vec!["evidence", "audit", "auth.login.01", "--ref", "x", "--audit-result", "confirmed", "--evaluator", "e", "--evaluator-version", "1", "--evaluator-digest", "sha256:00", "--role", "reviewer", "--json"], "EVIDENCE_INVALID_ARGUMENT"),
        ("review scenario: scenario no feature declares", vec!["review", "scenario", "nosuch.x.01", "--actor", "alice", "--role", "reviewer", "--original-spec-ref", "docs/p.md", "--rev", "HEAD", "--json"], "REVIEW_SCENARIO_NOT_INDEXED"),
        ("divergence accept: scenario no feature declares", vec!["divergence", "accept", "nosuch.x.01", "--actor", "alice", "--reason", "r", "--tracking", "t", "--json"], "DIVERGENCE_ARGUMENT_INVALID"),
    ]
}

#[test]
fn atip_readwrites_01_a_refused_call_leaves_the_repository_home_and_temp_dir_as_they_were_whatever_the_index() {
    let world = world();
    let before = world.snapshot();
    for (what, argv, code) in refused_calls() {
        let out = world.canon(&argv);
        let (exit, found) = code_of(&out);
        assert_eq!(exit, Some(2), "{what}: {}", String::from_utf8_lossy(&out.stdout));
        assert!(code.is_empty() && !found.is_empty() || found == code, "{what}: {found}");
        assert_eq!(world.snapshot(), before, "{what}: a refused call wrote");
        assert!(!world.indexed(), "{what}: a refused call refreshed the scenario index");
    }
    // text mode says nothing about a refresh either
    let out = world.canon(&["subject", "show", "nosuch"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("refreshed the scenario index"), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(world.snapshot(), before);
}

#[test]
fn atip_readwrites_02_a_call_that_is_accepted_refreshes_the_index_it_owes() {
    // subject show, a read command: the cache refresh is its declared effect
    let world_show = world();
    let out = world_show.canon(&["subject", "show", "demo", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(world_show.indexed(), "subject show refreshed nothing");
    assert!(out.stderr.is_empty(), "JSON mode writes no line to stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(json_of(&out)["ok"], true);

    // subject move
    let world_move = world();
    let out = world_move.canon(&["subject", "move", "demo", "--to", "specced", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(world_move.indexed(), "subject move refreshed nothing");

    // evidence add on a scenario only the feature file declares: the call knows it from the index it computes in memory
    let world_evidence = world();
    let out = world_evidence.canon(&["evidence", "add", "--project", "root", "--all-scenarios", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--report", "junit:reports/match.xml", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(world_evidence.indexed(), "evidence add refreshed nothing");

    // evidence audit, review scenario
    let world_audit = world();
    let out = world_audit.canon(&["evidence", "audit", "auth.login.01", "--ref", "x", "--audit-result", "confirmed", "--evaluator", "e", "--evaluator-version", "1", "--evaluator-digest", DIGEST, "--role", "reviewer", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(world_audit.indexed(), "evidence audit refreshed nothing");
    let world_review = world();
    let out = world_review.canon(&["review", "scenario", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--original-spec-ref", "docs/p.md", "--rev", "HEAD", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(world_review.indexed(), "review scenario refreshed nothing");

    // divergence accept: a divergent attestation by someone else, then a stale index again
    let world_accept = world();
    world_accept.ok(&["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "x", "--role", "implementer", "--verdict", "divergent", "--actor", "impl-1", "--session", "s-impl"]);
    world_accept.ok(&["scenario", "new", "auth.login.03", "--title", "Reset", "--feature", FEATURE]);
    std::fs::remove_dir_all(world_accept.scenario_index()).unwrap();
    let out = world_accept.canon(&["divergence", "accept", "auth.login.01", "--actor", "alice", "--session", "s-rev", "--reason", "known gap", "--tracking", "#1", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(world_accept.indexed(), "divergence accept refreshed nothing");

    // artifacts store (nothing to store: the pass still refreshes), and not when it only previews
    let world_store = world();
    let out = world_store.canon(&["artifacts", "store", "--dry-run", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!world_store.indexed(), "a preview reads only");
    let out = world_store.canon(&["artifacts", "store", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(world_store.indexed(), "artifacts store refreshed nothing");
}

#[test]
fn atip_readwrites_03_subject_show_says_its_cache_effect_and_is_quiet_on_stderr_in_json() {
    let world = world();
    let schema = json_of(&world.canon(&["schema", "subject", "show", "--json"]));
    assert_eq!(schema["data"]["effect"], "read", "{schema}");
    let note = schema["data"]["effectNote"].as_str().unwrap_or_default();
    assert!(note.contains("scenario-index") && note.contains("refresh"), "the effectNote names the index refresh: {note}");
    assert!(note.contains("refused call writes nothing"), "and says a refusal writes nothing: {note}");

    // JSON: the refresh is a warning in `meta`, never a line on stderr
    let out = world.canon(&["subject", "show", "demo", "--json"]);
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
    let warnings = json_of(&out)["meta"]["warnings"].clone();
    assert!(warnings.as_array().is_some_and(|all| all.iter().any(|w| w["code"] == "SCENARIO_INDEX_REFRESHED")), "{warnings}");

    // text: the progress line stays on stderr, once; a second call has nothing stale to say
    let text = world_stale_text();
    assert!(text.contains("refreshed the scenario index"), "{text}");
    let again = world.canon(&["subject", "show", "demo"]);
    assert!(!String::from_utf8_lossy(&again.stderr).contains("refreshed the scenario index"), "{}", String::from_utf8_lossy(&again.stderr));
}

fn world_stale_text() -> String {
    let world = world();
    String::from_utf8_lossy(&world.canon(&["subject", "show", "demo"]).stderr).to_string()
}

/// atip.readwrites.08 (the failure path of F2): `subject show` of a subject that does not exist is exit 2 `SUBJECT_NOT_FOUND` with one
/// envelope on stdout and nothing on stderr in JSON mode, and it neither refreshes the stale scenario index nor writes anything.
#[test]
fn atip_readwrites_08_subject_show_of_a_missing_subject_is_refused_quietly_and_writes_nothing() {
    let world = world();
    let before = world.snapshot();
    let out = world.canon(&["subject", "show", "nosuch", "--json"]);
    assert_eq!(code_of(&out), (Some(2), "SUBJECT_NOT_FOUND".to_string()), "{}", String::from_utf8_lossy(&out.stdout));
    assert!(out.stderr.is_empty(), "a refusal in JSON mode says nothing on stderr: {}", String::from_utf8_lossy(&out.stderr));
    let body = json_of(&out);
    assert!(body["meta"]["warnings"].as_array().is_none_or(|all| all.iter().all(|w| w["code"] != "SCENARIO_INDEX_REFRESHED")), "{body}");
    assert_eq!(world.snapshot(), before, "a refused call wrote");
    assert!(!world.indexed(), "a refused call refreshed the scenario index");
    // The subject that exists is not refused by the same stale repository: the refusal is about the subject, not the index.
    let found = world.canon(&["subject", "show", "demo", "--json"]);
    assert_eq!(found.status.code(), Some(0), "{}", String::from_utf8_lossy(&found.stdout));
}

#[test]
fn atip_readwrites_04_every_dry_run_help_version_schema_and_skills_show_writes_nothing() {
    let world = world();
    let before = world.snapshot();
    let adopt = ["--change", "add-login"];
    let previews: Vec<Vec<&str>> = vec![
        vec!["artifacts", "store"],
        vec!["export", "--kind", "change"],
        vec!["gate", "install-hooks"],
        vec!["gate", "promote"],
        vec!["ingest", "plans"],
        vec!["ingest", "records"],
        vec!["inventory", "sync"],
        vec!["skills", "install"],
        vec!["subject", "adopt", "demo", adopt[0], adopt[1]],
        vec!["subject", "unadopt", "demo", adopt[0], adopt[1]],
        vec!["plugin", "sync", "porting"],
        vec!["strategy", "promote", "demo"],
    ];
    let mut previewed = 0;
    for preview in &previews {
        let mut argv = preview.clone();
        argv.extend(["--dry-run", "--json"]);
        let out = world.canon(&argv);
        if out.status.success() {
            previewed += 1;
            assert_eq!(json_of(&out)["data"]["dryRun"], true, "{argv:?}");
        }
        assert_eq!(world.snapshot(), before, "{argv:?} wrote");
    }
    assert!(previewed >= 10, "the previews that reach their work: {previewed}");
    // ingest sessions reads `HOME`: the private one is empty
    let sessions = world.home.path().join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let before = world.snapshot();
    let out = world.canon(&["ingest", "sessions", "--home", sessions.to_str().unwrap(), "--all-workspaces", "--dry-run", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    assert_eq!(world.snapshot(), before, "ingest sessions --dry-run wrote");
    // init previews in an empty directory, and `canon init` really would write there
    let empty = TempDir::new().unwrap();
    let out = world.canon_in(empty.path(), &["init", "--providers", "none", "--dry-run", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    assert_eq!(std::fs::read_dir(empty.path()).unwrap().count(), 0, "init --dry-run wrote");
    assert_eq!(world.snapshot(), before);

    // --help, --version, schema and skills show, for every command the contract names
    let leaves: Vec<String> = {
        let schema = json_of(&world.canon(&["schema", "--json"]));
        schema["data"]["commands"].as_array().expect("schema lists the commands").iter().map(|c| c["command"].as_str().expect("a command name").to_string()).collect()
    };
    assert!(leaves.len() > 100, "{} commands", leaves.len());
    for leaf in &leaves {
        let words: Vec<&str> = leaf.split(' ').collect();
        let mut help = words.clone();
        help.push("--help");
        world.canon(&help);
        let mut schema = vec!["schema"];
        schema.extend(&words);
        schema.push("--json");
        world.canon(&schema);
    }
    world.canon(&["--version"]);
    world.canon(&["--help"]);
    for topic in ["canon", "role-implementer", "role-reviewer", "role-planner", "role-orchestrator"] {
        world.canon(&["skills", "show", topic]);
        world.canon(&["skills", "show", topic, "--json"]);
    }
    assert_eq!(world.snapshot(), before, "a help, version, schema or skills show call wrote");
}

#[test]
fn atip_readwrites_05_report_check_and_selftest_leave_no_file_in_the_temp_dir() {
    let world = world();
    let tmp_listing = |world: &World| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(world.tmp.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        names
    };
    assert!(tmp_listing(&world).is_empty());
    // success, refusal and failure of each
    let calls: Vec<Vec<&str>> = vec![
        vec!["report", "check", "--json"],
        vec!["report", "check", "--repo", "/no/such/repo", "--json"],
        vec!["selftest", "--json"],
        vec!["selftest", "--suite", "no-such-suite", "--json"],
    ];
    for argv in &calls {
        world.canon(argv);
        assert_eq!(tmp_listing(&world), Vec::<String>::new(), "{argv:?} left a file in the temporary directory");
    }
    // a failing check does not leave one either: a policy that cannot be read
    std::fs::write(world.repo().join("canon.yaml"), "tiers: [not, a, map\n").unwrap();
    for argv in [vec!["report", "check", "--json"], vec!["selftest", "--json"]] {
        world.canon(&argv);
        assert_eq!(tmp_listing(&world), Vec::<String>::new(), "{argv:?} left a file in the temporary directory (broken canon.yaml)");
    }
}

#[test]
fn atip_readwrites_06_dashboard_is_declared_a_server_that_serves_until_signalled() {
    let world = world();
    let schema = json_of(&world.canon(&["schema", "dashboard", "--json"]));
    let note = schema["data"]["effectNote"].as_str().unwrap_or_default();
    assert!(note.contains("serves until signalled"), "{note}");
    assert!(note.contains("one report"), "one report, then the server: {note}");
    assert!(!note.contains("stream"), "not a stream: {note}");
    // one report: the command is declared by its output type, and the run prints one envelope before it serves (a process that serves is not run here)
    assert_eq!(schema["data"]["effect"], "write", "{schema}");
    let outputs = schema["data"]["outputs"].as_array().expect("outputs");
    assert_eq!(outputs.len(), 1, "one output, no stream lines: {outputs:?}");
    assert_eq!((outputs[0]["kind"].as_str(), outputs[0]["schemaRef"].as_str()), (Some("report"), Some("#/schemas/DashboardReport")), "{outputs:?}");
}

/// atip.readwrites.09 (the failure path of F5): a `dashboard` call that is refused (a port that is no integer, an option it does not
/// have) is one error envelope with exit 2 on stdout, comes back at once instead of serving, and writes nothing.
#[test]
fn atip_readwrites_09_a_refused_dashboard_call_is_one_envelope_and_serves_nothing() {
    let world = world();
    let before = world.snapshot();
    for (argv, field) in [(&["dashboard", "--port", "notaport", "--json"][..], "--port"), (&["dashboard", "--bogus", "--json"][..], "--bogus")] {
        let out = world.canon(argv);
        assert_eq!(code_of(&out), (Some(2), "USAGE".to_string()), "{argv:?}: {}", String::from_utf8_lossy(&out.stdout));
        let body = json_of(&out);
        assert_eq!(body["error"]["field"], field, "{argv:?}: {body}");
        assert!(out.stderr.is_empty(), "{argv:?}: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(world.snapshot(), before, "{argv:?}: a refused call wrote (the dashboard snapshot is written only when it serves)");
    }
}

#[test]
fn atip_readwrites_07_artifacts_store_refused_for_an_unwritable_store_leaves_a_stale_index_alone() {
    use std::os::unix::fs::PermissionsExt as _;

    let world = world();
    std::fs::write(world.repo().join("out.txt"), "a bound file\n").unwrap();
    world.ok(&["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "x", "--role", "implementer", "--verdict", "faithful", "--artifact", "out.txt"]);
    // the blob is gone and the index stale again: the pass has a file to store, and the store cannot take it
    std::fs::remove_dir_all(world.repo().join(".canon/artifacts")).unwrap();
    world.ok(&["scenario", "new", "auth.login.03", "--title", "Reset", "--feature", FEATURE]);
    let ledger_scenarios = world.scenario_index();
    let index_before: Vec<_> = std::fs::read_dir(&ledger_scenarios).map(|entries| entries.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    let canon_dir = world.repo().join(".canon");
    let mode = std::fs::metadata(&canon_dir).unwrap().permissions().mode();
    std::fs::set_permissions(&canon_dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let before = world.snapshot();
    let out = world.canon(&["artifacts", "store", "--json"]);
    let refused = (out.status.code(), json_of(&out)["error"]["code"].as_str().unwrap_or_default().to_string());
    let after = world.snapshot();
    std::fs::set_permissions(&canon_dir, std::fs::Permissions::from_mode(mode)).unwrap();
    assert_eq!(refused.0, Some(2), "{refused:?}: {}", String::from_utf8_lossy(&out.stdout));
    assert_eq!(after, before, "a refused artifacts store wrote");
    let index_after: Vec<_> = std::fs::read_dir(&ledger_scenarios).map(|entries| entries.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    assert_eq!(index_after, index_before, "the scenario index was refreshed by a refused call");
}
