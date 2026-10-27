//! T12 (0.28 D6, D7): refusals write nothing, idempotent writes are no-ops the
//! second time, and `--dry-run` refuses like the real run.
//!
//! - [`REFUSALS`]: one recipe per write/destructive leaf, a refusal the leaf
//!   itself decides after parsing (an id or file that does not exist, a
//!   precondition that is unmet, an invalid value the runtime checks). Run on
//!   a copy of the fixture it exits with the recipe's code, and the whole tree
//!   (`.git` excluded) and `git status --porcelain` are as they were. A table
//!   row is required for every write leaf of the spec;
//! - a refused leaf that refreshes the scenario index first
//!   (`effectNote: refreshes stale scenario-index records first`) is held to
//!   that note: on the fixture, whose index is stale, it may touch nothing
//!   but the index's own records, and only a leaf that declares the refresh;
//! - every write leaf that needs canon state, pointed at a directory without
//!   `canon.yaml`, exits 2 `CONFIG_MISSING` and writes nothing, in either
//!   directory;
//! - every leaf with `--dry-run`: its recipe plus `--dry-run` refuses with the
//!   same exit and `error.code` and writes nothing;
//! - every leaf declared `replay-safe` or `content-keyed` runs its example
//!   twice and the second run changes nothing (and, when the report says
//!   `unchanged`, says true; a replay-safe or content-keyed leaf must say `meta.unchanged`, a destructive
//!   leaf's preview excepted); a `once` leaf refuses the second run. The state
//!   a leaf needs is built here; what cannot be built is listed in
//!   [`IDEMPOTENCY_NEEDS_STATE`] and checked to be honest. `appends` leaves
//!   (`demo review`, `dispatch begin`, `divergence accept|defer|resolve`) add
//!   a record per run by contract: there is nothing to hold them to.

mod harness;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Output;

use canon_cli::cli_error::all_codes;
use canon_cli::context::needs_config;
use harness::*;
use serde_json::Value;
use tempfile::TempDir;

/// What a recipe sets up in the copy of the fixture before it runs.
enum Setup {
    File(&'static str, &'static str),
    /// A directory where the leaf expects a file.
    Dir(&'static str),
    Remove(&'static str),
}

/// A recipe's command line, without `canon` and without `--json`. `@empty` is
/// a fresh empty directory and `@missing` a path that does not exist, both
/// outside the repo.
enum Argv {
    Lit(&'static [&'static str]),
    /// The leaf's first example (so the recipe follows the spec), then these.
    Example(&'static [&'static str]),
}

struct Row {
    leaf: &'static str,
    argv: Argv,
    setup: &'static [Setup],
    code: &'static str,
    exit: i32,
}

const fn lit(leaf: &'static str, argv: &'static [&'static str], code: &'static str) -> Row {
    Row { leaf, argv: Argv::Lit(argv), setup: &[], code, exit: 2 }
}

const fn example(leaf: &'static str, extra: &'static [&'static str], code: &'static str) -> Row {
    Row { leaf, argv: Argv::Example(extra), setup: &[], code, exit: 2 }
}

impl Row {
    const fn setup(mut self, setup: &'static [Setup]) -> Self {
        self.setup = setup;
        self
    }
}

/// A `canon.yaml` that does not parse: every leaf that reads the config refuses it.
const BROKEN_CONFIG: &[Setup] = &[Setup::File("canon.yaml", "tiers: [\n")];
const HOOKS_JSON_IS_A_DIRECTORY: &[Setup] = &[Setup::Dir(".codex/hooks.json")];
const NO_POLICY: &[Setup] = &[Setup::Remove(".canon/policy.yaml")];
const FEATURE: &str = ".canon/specs/kind=feature/area=auth/login.feature";
const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Every write and destructive leaf with a refusal it decides itself. A
/// `USAGE` row would be a leaf with nothing else to refuse; there is none.
#[rustfmt::skip]
const REFUSALS: &[Row] = &[
    lit("artifacts move", &["artifacts", "move", "--tier", "cold"], "ARTIFACTS_MOVE_REFUSED"),
    lit("artifacts store", &["artifacts", "store"], "CONFIG_UNREADABLE").setup(BROKEN_CONFIG),
    lit("brief approve", &["brief", "approve", "no-such-change", "--approver", "alice", "--note", "ok"], "BRIEF_NOT_FOUND"),
    lit("change new", &["change", "new", "add-login", "--subject-id", "demo", "--title", "Again"], "CHANGE_EXISTS"),
    lit("context-pack add", &["context-pack", "add", "--manifest", "missing.json"], "CONTEXT_INPUT_MISSING"),
    lit("dashboard", &["dashboard", "--repo", "@missing"], "REPO_UNREADABLE"),
    lit("demo init", &["demo", "init", "--repo", "canon.yaml"], "WRITE_FAILED"),
    lit("demo review", &["demo", "review", "--repo", "@empty"], "DEMO_NOT_INITIALIZED"),
    lit("dispatch begin", &["dispatch", "begin", "--regime", "implementer/my-repo/api/abcdef012345", "--role", "implementer", "--task-id", "add-login#1"], "TASK_NOT_FOUND"),
    lit("dispatch end", &["dispatch", "end", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "--run-outcome", "succeeded"], "DISPATCH_RUN_NOT_FOUND"),
    // The divergence leaves and `review scenario` take the commit option under whichever name the spec gives it: their recipe is the spec's example plus a spec root that does not exist.
    example("divergence accept", &["--project-id", "nosuchroot"], "UNKNOWN_SPEC_ROOT"),
    example("divergence add", &["--project-id", "nosuchroot"], "UNKNOWN_SPEC_ROOT"),
    example("divergence defer", &["--project-id", "nosuchroot"], "UNKNOWN_SPEC_ROOT"),
    example("divergence resolve", &["--project-id", "nosuchroot"], "UNKNOWN_SPEC_ROOT"),
    lit("evidence add", &["evidence", "add", "--scenario-id", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--artifact", "missing.bin"], "EVIDENCE_BIND_REFUSED"),
    lit("evidence audit", &["evidence", "audit", "auth.login.01", "--ref", "audit-report.json", "--evaluator-id", "eval-1", "--evaluator-version", "2.1", "--evaluator-digest", DIGEST, "--audit-result", "confirmed", "--role", "implementer", "--artifact", "missing.bin"], "EVIDENCE_BIND_REFUSED"),
    lit("export", &["export", "--kind", "evidence", "--output", "missing-dir/out.json"], "RETENTION_EXPORT_FAILED"),
    lit("feature new", &["feature", "new", "auth.login", "--title", "Again"], "FEATURE_EXISTS"),
    lit("finding add", &["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor-id", "alice", "--summary", "Again"], "FINDING_SEQ_OCCUPIED"),
    lit("finding close", &["finding", "close", "add-login", "--round", "9", "--seq", "9", "--disposition", "rejected", "--reason", "no such finding"], "FINDING_NOT_FOUND"),
    // `.claude/settings.json` is written first: a refusal on the second file must leave the first alone.
    lit("gate install-hooks", &["gate", "install-hooks"], "HOOKS_INSTALL_FAILED").setup(HOOKS_JSON_IS_A_DIRECTORY),
    lit("gate promote", &["gate", "promote"], "CONFIG_UNREADABLE").setup(BROKEN_CONFIG),
    lit("ingest plans", &["ingest", "plans", "--repo", "@empty"], "CONFIG_MISSING"),
    lit("ingest records", &["ingest", "records", "--repo", "@empty"], "CONFIG_MISSING"),
    lit("ingest sessions", &["ingest", "sessions", "--repo", "@empty"], "CONFIG_MISSING"),
    lit("init", &["init", "--providers", "bogus", "--repo", "@empty"], "INIT_INVALID_ARGUMENT"),
    lit("inventory sync", &["inventory", "sync"], "INVENTORY_FAILED").setup(BROKEN_CONFIG),
    lit("ledger compact", &["ledger", "compact", "--commit", "--plan-digest", "bogus"], "PLAN_CHANGED"),
    lit("migrate", &["migrate", "--commit", "--plan-digest", "bogus"], "PLAN_CHANGED"),
    lit("plugin sync", &["plugin", "sync", "no-such-plugin"], "PLUGIN_SYNC_FAILED"),
    lit("policy approve", &["policy", "approve", "--approver", "alice", "--note", "reviewed"], "POLICY_APPROVAL_FAILED").setup(NO_POLICY),
    lit("prompt add", &["prompt", "add", "reviewer@v1", "--manifest", "missing.json"], "CONTEXT_INPUT_MISSING"),
    lit("purge", &["purge", "--kind", "session", "--before", "2020-01-01T00:00:00Z", "--commit", "--plan-digest", "bogus"], "PLAN_CHANGED"),
    lit("report build", &["report", "build"], "CONFIG_UNREADABLE").setup(BROKEN_CONFIG),
    lit("report snapshot", &["report", "snapshot", "snapshot-out"], "CONFIG_UNREADABLE").setup(BROKEN_CONFIG),
    // The spec's example: `add-login` is a feature, and only a chore change is reviewed whole.
    example("review change", &[], "REVIEW_INVALID_ARGUMENT"),
    example("review scenario", &["--project-id", "nosuchroot"], "UNKNOWN_SPEC_ROOT"),
    lit("scenario new", &["scenario", "new", "auth.login.02", "--title", "Again", "--feature", FEATURE], "SCENARIO_TAG_EXISTS"),
    lit("skills install", &["skills", "install", "--providers", "bogus"], "SKILLS_INVALID_ARGUMENT"),
    lit("strategy promote", &["strategy", "promote", "01ARZ3NDEKTSV4RRFFQ69G5FAV"], "CONFIG_UNREADABLE").setup(BROKEN_CONFIG),
    lit("strategy rollback", &["strategy", "rollback", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "--reason", "regressed", "--actor-id", "alice", "--signature-file", "missing.sig", "--approval-at", "2030-01-01T00:00:00Z"], "LEARN_STRATEGY_NOT_FOUND"),
    lit("subject adopt", &["subject", "adopt", "no-such", "--change-id", "add-login"], "SUBJECT_ADOPT_REFUSED"),
    lit("subject move", &["subject", "move", "demo", "--to", "shipped"], "SUBJECT_INVALID_TRANSITION"),
    lit("subject new", &["subject", "new", "demo", "--domain", "dev", "--title", "Again"], "SUBJECT_EXISTS"),
    lit("subject unadopt", &["subject", "unadopt", "no-such", "--change-id", "add-login"], "SUBJECT_ADOPT_REFUSED"),
    lit("task done", &["task", "done", "add-login#1"], "TASK_NOT_FOUND"),
    lit("tier age", &["tier", "age", "--commit", "--plan-digest", "bogus"], "PLAN_CHANGED"),
];

/// Cases the checks catch that are not fixed: (leaf, check, what is wrong).
/// Each must still fail; a fix removes its entry.
const KNOWN_BUGS: &[(&str, &str, &str)] = &[];

/// Replay-safe, content-keyed or once leaves whose example cannot run to
/// success in the fixture, and the state they need.
const IDEMPOTENCY_NEEDS_STATE: &[(&str, &str)] = &[(
    "strategy rollback",
    "a human's signed approval: an ssh key pinned as an allowed signer in .canon/policy.yaml, and a signature over the rollback payload, which needs ssh-keygen",
)];

/// Declared-idempotent leaves not run twice at all.
const IDEMPOTENCY_EXCLUDED: &[(&str, &str)] = &[("dashboard", "binds a port and serves until the process is stopped")];

/// Paths a second run of a replay-safe leaf may rewrite, by the leaf's own declaration: (leaf, path prefix, why).
const IDEMPOTENCY_ALLOWED_DIFF: &[(&str, &str, &str)] = &[
    ("export", ".canon/audit/", "effectNote: one access-audit line per run"),
];

fn write_leaves() -> Vec<&'static Value> {
    leaves_with_effect(&["write", "destructive"])
}

fn error_codes(node: &Value) -> Vec<String> {
    strings(&node["errorCodes"])
}

fn has_option(node: &Value, name: &str) -> bool {
    option(node, name).is_some()
}

/// A fixture whose index is up to date: the leaves that refresh it first then have nothing to write.
fn synced_fixture(home: &Path) -> TempDir {
    let base = fixture(home);
    ok(base.path(), home, &["inventory", "sync"]);
    base
}

/// A copy of a base, the recipe's setup applied, and a scratch directory next to it (`@empty`, `@missing`).
struct Trial {
    _guard: TempDir,
    repo: PathBuf,
    scratch: TempDir,
}

fn trial(base: &Path, setup: &[Setup]) -> Trial {
    let (guard, repo) = copy_of(base);
    for step in setup {
        match step {
            Setup::File(path, content) => {
                let target = repo.join(path);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(target, content).unwrap();
            }
            Setup::Dir(path) => {
                let target = repo.join(path);
                let _ = std::fs::remove_file(&target);
                std::fs::create_dir_all(target).unwrap();
            }
            Setup::Remove(path) => {
                let target = repo.join(path);
                if target.is_dir() {
                    std::fs::remove_dir_all(target).unwrap();
                } else {
                    std::fs::remove_file(target).unwrap();
                }
            }
        }
    }
    let scratch = TempDir::new().unwrap();
    std::fs::create_dir(scratch.path().join("empty")).unwrap();
    Trial { _guard: guard, repo, scratch }
}

struct Snap {
    repo: std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>,
    scratch: std::collections::BTreeMap<PathBuf, Option<Vec<u8>>>,
    status: String,
}

fn snap(t: &Trial) -> Snap {
    Snap { repo: tree(&t.repo), scratch: tree(t.scratch.path()), status: git(&t.repo, &["status", "--porcelain"]) }
}

/// What differs between two snapshots, as readable lines.
fn differences(before: &Snap, after: &Snap) -> Vec<String> {
    let mut out: Vec<String> = changed(&before.repo, &after.repo).iter().map(|p| format!("repo/{}", p.display())).collect();
    out.extend(changed(&before.scratch, &after.scratch).iter().map(|p| format!("scratch/{}", p.display())));
    if before.status != after.status {
        out.push(format!("git status: `{}` became `{}`", before.status.trim(), after.status.trim()));
    }
    out
}

fn resolve(tokens: &[String], t: &Trial) -> Vec<String> {
    let empty = t.scratch.path().join("empty").display().to_string();
    let missing = t.scratch.path().join("missing").display().to_string();
    tokens.iter().map(|token| token.replace("@empty", &empty).replace("@missing", &missing)).collect()
}

fn recipe_argv(row: &Row) -> Vec<String> {
    match &row.argv {
        Argv::Lit(tokens) => strs(tokens),
        Argv::Example(extra) => {
            let mut argv = example_argv(leaf(row.leaf));
            argv.extend(strs(extra));
            argv
        }
    }
}

fn error_code(document: &Value) -> String {
    document["error"]["code"].as_str().unwrap_or_default().to_string()
}

fn parse(out: &Output) -> Option<Value> {
    serde_json::from_slice(&out.stdout).ok()
}

/// Runs `argv` (plus `--json`) in the trial's repo.
fn run(t: &Trial, home: &Path, argv: &[String], envs: &[(&str, &str)]) -> Output {
    let mut argv = resolve(argv, t);
    argv.push("--json".into());
    canon_in(&t.repo, home, &argv, envs)
}

/// Applies the known-bug entries of `check` to what the check found: an unlisted problem, or a listed bug that no longer shows, fails.
fn settle(check: &str, found: Vec<(String, String)>) {
    let known: Vec<&(&str, &str, &str)> = KNOWN_BUGS.iter().filter(|(_, c, _)| *c == check).collect();
    let mut unexpected = Vec::new();
    for (leaf, problem) in &found {
        if !known.iter().any(|(l, _, _)| l == leaf) {
            unexpected.push(format!("{leaf}: {problem}"));
        }
    }
    for (leaf, _, what) in &known {
        if !found.iter().any(|(l, _)| l == leaf) {
            unexpected.push(format!("{leaf}: the known bug no longer shows ({what}); remove it from KNOWN_BUGS"));
        }
    }
    assert!(unexpected.is_empty(), "{check}:\n{}", unexpected.join("\n"));
}

// ---------------------------------------------------------------------------
// The table.

#[test]
fn every_write_leaf_has_one_refusal_recipe_naming_a_code_it_lists() {
    let spec_leaves: BTreeSet<String> = write_leaves().iter().map(|n| path_of(n)).collect();
    let rows: Vec<&str> = REFUSALS.iter().map(|r| r.leaf).collect();
    let table: BTreeSet<String> = rows.iter().map(std::string::ToString::to_string).collect();
    assert_eq!(rows.len(), table.len(), "a leaf has two recipes: {rows:?}");
    assert_eq!(table, spec_leaves, "REFUSALS and the spec's write and destructive leaves differ (a new write leaf needs a row)");

    let registry = all_codes();
    let mut problems = Vec::new();
    for row in REFUSALS {
        let node = leaf(row.leaf);
        if !error_codes(node).iter().any(|c| c == row.code) {
            problems.push(format!("{}: the spec does not list `{}` among {:?}", row.leaf, row.code, error_codes(node)));
        }
        match registry.iter().find(|info| info.code == row.code) {
            Some(info) if i32::from(info.exit) == row.exit => {}
            Some(info) => problems.push(format!("{}: `{}` exits {} in the registry, the row says {}", row.leaf, row.code, info.exit, row.exit)),
            None => problems.push(format!("{}: `{}` is not a registered code", row.leaf, row.code)),
        }
        let argv = recipe_argv(row);
        let words: Vec<&str> = row.leaf.split(' ').collect();
        if argv.iter().take(words.len()).map(String::as_str).ne(words.iter().copied()) {
            problems.push(format!("{}: the recipe does not invoke it: {argv:?}", row.leaf));
        }
        if argv.iter().any(|a| a == "--json") {
            problems.push(format!("{}: the recipe carries --json, the test adds it", row.leaf));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// ---------------------------------------------------------------------------
// A refused write writes nothing.

#[test]
fn a_refused_write_leaves_the_tree_and_git_status_unchanged() {
    let home = TempDir::new().unwrap();
    let base = synced_fixture(home.path());
    let mut found = Vec::new();
    for row in REFUSALS {
        let t = trial(base.path(), row.setup);
        let before = snap(&t);
        let out = run(&t, home.path(), &recipe_argv(row), &[]);
        let after = snap(&t);
        let document = parse(&out);
        let code = document.as_ref().map(error_code).unwrap_or_default();
        if out.status.code() != Some(row.exit) || code != row.code || document.as_ref().is_some_and(|d| d["ok"] != false) {
            found.push((row.leaf.to_string(), format!("exit {:?} `{code}`, expected {} `{}`: {}", out.status.code(), row.exit, row.code, String::from_utf8_lossy(&out.stdout))));
            continue;
        }
        let diff = differences(&before, &after);
        if !diff.is_empty() {
            found.push((row.leaf.to_string(), format!("refused with {} but wrote: {diff:?}", row.code)));
        }
    }
    settle("refusal", found);
}

/// A leaf may refresh the scenario index before it refuses, only when its `effectNote` says so.
#[test]
fn a_refusal_before_which_the_index_is_refreshed_is_declared() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let mut found = Vec::new();
    let mut refreshed = BTreeSet::new();
    for row in REFUSALS {
        let t = trial(base.path(), row.setup);
        let before = snap(&t);
        let _ = run(&t, home.path(), &recipe_argv(row), &[]);
        let after = snap(&t);
        let paths = changed(&before.repo, &after.repo);
        if paths.is_empty() && before.status == after.status {
            continue;
        }
        let index_only = paths.iter().all(|p| p.starts_with(".canon/ledger/kind=scenario") || p.starts_with(".canon/archive/kind=scenario"));
        let declared = leaf(row.leaf)["effectNote"].as_str().is_some_and(|note| note.contains("refreshes stale scenario-index records"));
        if !index_only || before.status != after.status {
            // Writes of another kind are the previous test's: it runs on a fresh index.
            continue;
        }
        refreshed.insert(row.leaf);
        if !declared {
            found.push((row.leaf.to_string(), format!("refreshed the scenario index before refusing {} and does not declare it in its effectNote: {paths:?}", row.code)));
        }
    }
    assert!(!refreshed.is_empty(), "no recipe reached an index refresh: the fixture's index is not stale any more");
    settle("stale-index", found);
}

// ---------------------------------------------------------------------------
// The missing precondition.

#[test]
fn a_write_leaf_that_needs_canon_state_refuses_a_directory_without_a_config() {
    let home = TempDir::new().unwrap();
    let base = synced_fixture(home.path());
    let mut found = Vec::new();
    let mut checked = 0;
    for node in write_leaves() {
        let name = path_of(node);
        if !has_option(node, "repo") || !needs_config(&name) {
            continue;
        }
        checked += 1;
        let t = trial(base.path(), &[]);
        let empty = t.scratch.path().join("empty").display().to_string();
        let mut argv = example_argv(node);
        match argv.iter().position(|a| a == "--repo") {
            Some(at) if at + 1 < argv.len() => argv[at + 1] = empty,
            _ => argv.extend(["--repo".to_string(), empty]),
        }
        let before = snap(&t);
        let out = run(&t, home.path(), &argv, &[]);
        let after = snap(&t);
        let code = parse(&out).as_ref().map(error_code).unwrap_or_default();
        if out.status.code() != Some(2) || code != "CONFIG_MISSING" {
            found.push((name, format!("exit {:?} `{code}`: {}", out.status.code(), String::from_utf8_lossy(&out.stdout))));
            continue;
        }
        let diff = differences(&before, &after);
        if !diff.is_empty() {
            found.push((name, format!("refused CONFIG_MISSING but wrote: {diff:?}")));
        }
    }
    assert!(checked >= 25, "only {checked} leaves were swept");
    settle("config-missing", found);
}

// ---------------------------------------------------------------------------
// --dry-run.

#[test]
fn a_dry_run_refuses_with_the_code_of_the_real_run_and_writes_nothing() {
    let home = TempDir::new().unwrap();
    let base = synced_fixture(home.path());
    let mut found = Vec::new();
    let mut checked = BTreeSet::new();
    for row in REFUSALS {
        if !has_option(leaf(row.leaf), "dry-run") {
            continue;
        }
        checked.insert(row.leaf);
        let t = trial(base.path(), row.setup);
        let before = snap(&t);
        let mut argv = recipe_argv(row);
        argv.push("--dry-run".into());
        let out = run(&t, home.path(), &argv, &[]);
        let after = snap(&t);
        let code = parse(&out).as_ref().map(error_code).unwrap_or_default();
        if out.status.code() != Some(row.exit) || code != row.code {
            found.push((row.leaf.to_string(), format!("the dry run exits {:?} `{code}`, the real run {} `{}`: {}", out.status.code(), row.exit, row.code, String::from_utf8_lossy(&out.stdout))));
            continue;
        }
        let diff = differences(&before, &after);
        if !diff.is_empty() {
            found.push((row.leaf.to_string(), format!("the dry run refused with {code} but wrote: {diff:?}")));
        }
    }
    let with_dry_run: BTreeSet<&str> = leaves().into_iter().filter(|n| has_option(n, "dry-run")).map(|n| n["command"].as_str().unwrap()).collect();
    assert_eq!(checked, with_dry_run, "every leaf with --dry-run has a recipe");
    assert!(checked.len() >= 14, "{checked:?}");
    settle("dry-run", found);
}

// ---------------------------------------------------------------------------
// Idempotency.

#[test]
fn an_idempotency_exception_names_a_declared_idempotent_leaf_that_cannot_run() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let declared = declared_idempotent();
    for (name, state) in IDEMPOTENCY_NEEDS_STATE {
        assert!(declared.contains(*name), "`{name}` is not a replay-safe, content-keyed or once write leaf");
        let c = case(name, base.path(), home.path());
        let out = run_case(&c, home.path());
        assert_ne!(out.status.code(), Some(0), "`{name}` now runs to success ({state}): remove it from IDEMPOTENCY_NEEDS_STATE");
    }
    for (name, _) in IDEMPOTENCY_EXCLUDED {
        assert!(declared.contains(*name), "`{name}` is not a declared-idempotent write leaf");
    }
    for (name, _, _) in IDEMPOTENCY_ALLOWED_DIFF {
        assert!(declared.contains(*name), "`{name}` is not a declared-idempotent write leaf");
    }
}

/// The write leaves that declare `replay-safe`, `content-keyed` or `once`.
fn declared_idempotent() -> BTreeSet<&'static str> {
    write_leaves().into_iter().filter(|n| matches!(n["idempotency"].as_str(), Some("replay-safe" | "content-keyed" | "once"))).map(|n| n["command"].as_str().unwrap()).collect()
}

#[test]
fn a_declared_idempotent_write_leaf_is_a_no_op_the_second_time() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path());
    let mut found = Vec::new();
    let mut ran = BTreeSet::new();
    let mut used_allowance = BTreeSet::new();
    for node in write_leaves() {
        let name = path_of(node);
        let kind = node["idempotency"].as_str().unwrap_or_default();
        if kind == "appends" {
            continue;
        }
        assert!(matches!(kind, "replay-safe" | "content-keyed" | "once"), "`{name}` declares `{kind}`");
        if IDEMPOTENCY_NEEDS_STATE.iter().chain(IDEMPOTENCY_EXCLUDED).any(|(l, _)| *l == name) {
            continue;
        }
        let c = case(&name, base.path(), home.path());
        let first = run_case(&c, home.path());
        if first.status.code() != Some(0) {
            found.push((name.clone(), format!("the example does not run to success: exit {:?}: {}", first.status.code(), String::from_utf8_lossy(&first.stdout))));
            continue;
        }
        ran.insert(name.clone());
        let after_first = tree(&c.dir);
        let second = run_case(&c, home.path());
        let mut diff: Vec<PathBuf> = changed(&after_first, &tree(&c.dir));
        // sqlite's own files (the db, its WAL and shared-memory index) change with every open and close of the hot tier, whatever the rows say.
        diff.retain(|p| !p.to_string_lossy().starts_with(".canon/hot.db"));
        for (leaf_name, prefix, _) in IDEMPOTENCY_ALLOWED_DIFF {
            if *leaf_name == name {
                let before = diff.len();
                diff.retain(|p| !p.starts_with(prefix));
                if diff.len() != before {
                    used_allowance.insert(*leaf_name);
                }
            }
        }
        if kind == "once" {
            if second.status.code() != Some(2) {
                found.push((name.clone(), format!("a `once` leaf: the second run exits {:?}, not 2: {}", second.status.code(), String::from_utf8_lossy(&second.stdout))));
            }
        } else {
            if second.status.code() != Some(0) {
                found.push((name.clone(), format!("the second run exits {:?}: {}", second.status.code(), String::from_utf8_lossy(&second.stdout))));
                continue;
            }
            // A replay-safe or content-keyed leaf says its no-op: `meta.unchanged` is true. A destructive
            // leaf's example is its bare preview, which is a read of the plan and not a no-op: its
            // `--commit` repeat is held to `unchanged` by `contract_d5.rs`.
            match parse(&second) {
                _ if node["effect"] == "destructive" => {}
                Some(document) if document["meta"]["unchanged"] == true => {}
                other => found.push((name.clone(), format!("a `{kind}` leaf: the second run does not report meta.unchanged = true: {}", other.map(|d| d["meta"].to_string()).unwrap_or_default()))),
            }
            // `unchanged` is the report's own word for a no-op: when it is there it is true.
            if let Some(document) = parse(&second) {
                for place in ["data", "meta"] {
                    if let Some(unchanged) = document[place].get("unchanged") {
                        if unchanged != true {
                            found.push((name.clone(), format!("the second run reports {place}.unchanged = {unchanged}")));
                        }
                    }
                }
            }
        }
        if !diff.is_empty() {
            found.push((name.clone(), format!("the second run changed {diff:?}")));
        }
    }
    for (name, prefix, _) in IDEMPOTENCY_ALLOWED_DIFF {
        assert!(used_allowance.contains(name), "`{name}` no longer rewrites {prefix}: remove it from IDEMPOTENCY_ALLOWED_DIFF");
    }
    let expected: BTreeSet<String> = declared_idempotent().into_iter().filter(|l| !IDEMPOTENCY_NEEDS_STATE.iter().chain(IDEMPOTENCY_EXCLUDED).any(|(n, _)| n == l)).map(str::to_string).collect();
    settle("idempotency", found);
    assert_eq!(ran, expected, "every declared-idempotent write leaf not in the allowlists was run twice");
}
