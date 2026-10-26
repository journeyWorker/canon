//! Shared helpers of the grammar test suite (`tests/grammar_t*.rs`, 0.28 T3-T12):
//! the generated spec as data, the per-leaf table (`docs/design/cli-commands-0.28.md`)
//! and the vocabulary (`docs/design/cli-vocabulary.tsv`) parsed, and a scratch
//! repository the leaves run in.
//!
//! Each test binary uses a subset, so dead-code warnings are expected here.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use serde_json::Value;
use tempfile::TempDir;

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The generated spec, in memory (T4 holds it equal to the checked-in file).
pub fn spec() -> &'static Value {
    static SPEC: LazyLock<Value> = LazyLock::new(canon_cli::spec::generate);
    &SPEC
}

/// A node's command path as `schema` lists it (`gate check`).
pub fn path_of(node: &Value) -> String {
    node["command"].as_str().expect("a node names its command").to_string()
}

/// Every leaf node of the spec.
pub fn leaves() -> Vec<&'static Value> {
    spec()["commands"].as_array().expect("commands").iter().filter(|n| n["nodeKind"] == "leaf").collect()
}

pub fn leaf(path: &str) -> &'static Value {
    leaves().into_iter().find(|n| path_of(n) == path).unwrap_or_else(|| panic!("`{path}` is not a leaf of the spec"))
}

/// Leaves whose spec `effect` is one of `effects`.
pub fn leaves_with_effect(effects: &[&str]) -> Vec<&'static Value> {
    leaves().into_iter().filter(|n| effects.contains(&n["effect"].as_str().unwrap_or_default())).collect()
}

pub fn options(node: &Value) -> &Vec<Value> {
    node["options"].as_array().expect("a leaf lists its options")
}

pub fn option<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    options(node).iter().find(|o| o["name"] == name)
}

pub fn strings(value: &Value) -> Vec<String> {
    value.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// Splits an example into words: whitespace separates; single and double quotes group.
pub fn words(line: &str) -> Vec<String> {
    let (mut out, mut current, mut quote, mut any) = (Vec::new(), String::new(), None::<char>, false);
    for c in line.chars() {
        match (c, quote) {
            ('"' | '\'', None) => {
                quote = Some(c);
                any = true;
            }
            (c, Some(q)) if c == q => quote = None,
            (c, None) if c.is_whitespace() => {
                if any {
                    out.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            (c, _) => {
                current.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(current);
    }
    out
}

/// A leaf's first example as argv without the leading `canon`.
pub fn example_argv(node: &Value) -> Vec<String> {
    let example = node["examples"][0].as_str().unwrap_or_else(|| panic!("`{}` has no example", path_of(node)));
    let mut argv = words(example);
    assert_eq!(argv.remove(0), "canon");
    argv
}

// ---------------------------------------------------------------------------
// The per-leaf table.

/// One option as the per-leaf table writes it: `--name <VALUE>{a,b}=default...`.
#[derive(Debug, Clone, Default)]
pub struct TableOption {
    pub name: String,
    pub value: Option<String>,
    pub set: Vec<String>,
    pub default: Option<String>,
    pub repeatable: bool,
    pub required: bool,
}

fn cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = line.trim().trim_start_matches('|').chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                current.push('|');
                chars.next();
            }
            '|' => cells.push(std::mem::take(&mut current).trim().to_string()),
            c => current.push(c),
        }
    }
    cells.push(current.trim().to_string());
    cells
}

/// Parses one backticked span of an options cell (`--limit <N>=5`); `None` when it is no option.
fn table_option(span: &str, required: bool) -> Option<TableOption> {
    let span = span.trim();
    let span = match span.split_once(", ") {
        Some((short, rest)) if short.len() == 2 && short.starts_with('-') => rest,
        _ => span,
    };
    let rest = span.strip_prefix("--")?;
    let end = rest.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')).unwrap_or(rest.len());
    let (name, mut rest) = rest.split_at(end);
    if name.is_empty() {
        return None;
    }
    let mut option = TableOption { name: name.to_string(), required, ..Default::default() };
    if let Some(after) = rest.trim_start().strip_prefix('<') {
        let (value, tail) = after.split_once('>')?;
        option.value = Some(value.to_string());
        rest = tail;
    }
    if let Some(after) = rest.strip_prefix('{') {
        let (set, tail) = after.split_once('}')?;
        option.set = set.split(',').map(str::to_string).collect();
        rest = tail;
    }
    if let Some(stripped) = rest.strip_suffix("...") {
        option.repeatable = true;
        rest = stripped;
    }
    if let Some(default) = rest.strip_prefix('=') {
        option.default = Some(default.trim_end_matches("...").to_string());
    }
    Some(option)
}

/// Every leaf row of the table: the options its required and optional columns name.
pub fn table() -> &'static BTreeMap<String, Vec<TableOption>> {
    static TABLE: LazyLock<BTreeMap<String, Vec<TableOption>>> = LazyLock::new(|| {
        let text = std::fs::read_to_string(repo_root().join("docs/design/cli-commands-0.28.md")).expect("read the per-leaf table");
        let mut rows = BTreeMap::new();
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("| `canon ") else { continue };
            let Some((command, _)) = rest.split_once("` |") else { continue };
            let columns = cells(line);
            if columns.len() < 8 {
                continue;
            }
            let mut options = Vec::new();
            for (column, required) in [(2, true), (3, false)] {
                for (i, span) in columns[column].split('`').enumerate() {
                    if i % 2 == 1 {
                        options.extend(table_option(span, required));
                    }
                }
            }
            rows.insert(command.to_string(), options);
        }
        rows
    });
    &TABLE
}

// ---------------------------------------------------------------------------
// The vocabulary.

/// One row of `docs/design/cli-vocabulary.tsv`.
#[derive(Debug, Clone)]
pub struct VocabRow {
    pub option: String,
    /// `None` for a boolean (the TSV writes `-`).
    pub value_name: Option<String>,
    pub concept: String,
}

pub fn vocabulary() -> &'static Vec<VocabRow> {
    static VOCAB: LazyLock<Vec<VocabRow>> = LazyLock::new(|| {
        let text = std::fs::read_to_string(repo_root().join("docs/design/cli-vocabulary.tsv")).expect("read the vocabulary");
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("option\tvalue_name\tconcept"), "the vocabulary's header");
        lines
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let columns: Vec<&str> = line.split('\t').collect();
                assert_eq!(columns.len(), 3, "a vocabulary row has three tab-separated columns: {line:?}");
                VocabRow { option: columns[0].to_string(), value_name: (columns[1] != "-").then(|| columns[1].to_string()), concept: columns[2].to_string() }
            })
            .collect()
    });
    &VOCAB
}

// ---------------------------------------------------------------------------
// Running canon.

/// `canon <args>` in `dir` with a fixed environment (`HOME` is `home`; no ambient `CANON_*`).
pub fn canon_in(dir: &Path, home: &Path, args: &[String], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command
        .args(args)
        .current_dir(dir)
        .env_remove("CANON_ROLE")
        .env("CANON_ACTOR", "grammar-tests")
        .env("HOME", home)
        .env("CANON_SKILLS_SOURCE", repo_root().join("canon/skills"));
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("spawn the canon binary")
}

pub fn strs(args: &[&str]) -> Vec<String> {
    args.iter().map(std::string::ToString::to_string).collect()
}

/// `canon <args>` that must exit 0.
pub fn ok(dir: &Path, home: &Path, args: &[&str]) -> Output {
    let out = canon_in(dir, home, &strs(args), &[]);
    assert!(out.status.success(), "canon {args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    out
}

/// The JSON document on stdout.
pub fn document(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

/// Background maintenance is off: `git commit` may start `git gc --auto`, which deletes files while a tree is walked.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "gc.auto=0", "-c", "maintenance.auto=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A git repo with the story every example tells: subject `demo`, change `add-login`,
/// scenario `auth.login.01` (and `.02`, after the last index sync), finding 1, one
/// evidence record; committed.
pub fn fixture(home: &Path) -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "."]);
    ok(repo, home, &["init", "--providers", "none"]);
    ok(repo, home, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, home, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    ok(repo, home, &["feature", "new", "auth.login", "--title", "Login"]);
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    ok(repo, home, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", feature]);
    ok(repo, home, &["inventory", "sync"]);
    // Records about a commit need one.
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "seed"]);
    ok(repo, home, &["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor-id", "bob", "--summary", "Naming nit"]);
    ok(
        repo,
        home,
        &["evidence", "add", "--project-id", "root", "--scenario-id", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--verdict", "faithful", "--summary", "1 passed", "--actor-id", "impl", "--session-id", "s1", "--role", "implementer"],
    );
    ok(repo, home, &["scenario", "new", "auth.login.02", "--title", "Sign out", "--feature", feature]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    dir
}

pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every file under `root` (`.git` excluded) with its content; `None` marks a directory.
pub fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if relative == Path::new(".git") {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                out.insert(relative, None);
                walk(root, &path, out);
            } else {
                out.insert(relative, std::fs::read(&path).ok());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// The paths whose presence or content differs.
pub fn changed(before: &BTreeMap<PathBuf, Option<Vec<u8>>>, after: &BTreeMap<PathBuf, Option<Vec<u8>>>) -> Vec<PathBuf> {
    let paths: BTreeSet<&PathBuf> = before.keys().chain(after.keys()).collect();
    paths.into_iter().filter(|p| before.get(*p) != after.get(*p)).cloned().collect()
}

/// A copy of `base` in a fresh directory; keep the guard alive while using the path.
pub fn copy_of(base: &Path) -> (TempDir, PathBuf) {
    let scratch = TempDir::new().unwrap();
    let repo = scratch.path().join("repo");
    copy_dir(base, &repo);
    (scratch, repo)
}

/// Runs `args` on a copy of `base`; returns the paths it changed and the output.
pub fn run_on_copy(base: &Path, home: &Path, args: &[String]) -> (Vec<PathBuf>, Output) {
    let (_guard, repo) = copy_of(base);
    let before = tree(&repo);
    let out = canon_in(&repo, home, args, &[]);
    (changed(&before, &tree(&repo)), out)
}

// ---------------------------------------------------------------------------
// Validating a document against the spec's schemas.

fn compile(schema: &Value) -> jsonschema::Validator {
    jsonschema::validator_for(schema).unwrap_or_else(|e| panic!("a schema of the spec does not compile: {e}"))
}

/// What `canon --json` printed must be: an envelope (`ok`, `data`/`error`, `meta`) whose `data`
/// is one of the reports its leaf declares in `outputs`. Returns the problems, empty when valid.
pub fn validate_document(document: &Value) -> Vec<String> {
    use std::collections::HashMap;
    use std::sync::Mutex;

    static VALIDATORS: LazyLock<Mutex<HashMap<String, std::sync::Arc<jsonschema::Validator>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let validator = |key: &str, schema: &Value| -> std::sync::Arc<jsonschema::Validator> {
        VALIDATORS.lock().unwrap().entry(key.to_string()).or_insert_with(|| std::sync::Arc::new(compile(schema))).clone()
    };

    let mut problems = Vec::new();
    let ok = document["ok"].as_bool();
    let envelope = if ok == Some(true) { "success" } else { "failure" };
    let schema = &spec()["envelopes"][envelope];
    for e in validator(&format!("envelope:{envelope}"), schema).iter_errors(document) {
        problems.push(format!("the {envelope} envelope: {e} at {}", e.instance_path()));
    }
    let Some(command) = document["meta"]["command"].as_str() else {
        problems.push("meta.command is missing".into());
        return problems;
    };
    let Some(node) = leaves().into_iter().find(|n| path_of(n) == command) else {
        problems.push(format!("meta.command `{command}` is not a leaf of the spec"));
        return problems;
    };
    let Some(data) = document.get("data") else {
        if ok == Some(true) {
            problems.push("a success without data".into());
        }
        return problems;
    };
    let outputs = node["outputs"].as_array().expect("outputs");
    let mut best: Vec<String> = Vec::new();
    for output in outputs {
        let name = output["report"].as_str().expect("report");
        let Some(schema) = spec()["schemas"].get(name) else { continue };
        let errors: Vec<String> = validator(&format!("report:{name}"), schema).iter_errors(data).map(|e| format!("{e} at {}", e.instance_path())).collect();
        if errors.is_empty() {
            let declared = output["schemaVersion"].as_str();
            if declared != document["meta"]["schemaVersion"].as_str() {
                problems.push(format!("meta.schemaVersion is {} but {name} declares {declared:?}", document["meta"]["schemaVersion"]));
            }
            return problems;
        }
        if best.is_empty() || errors.len() < best.len() {
            best = errors.into_iter().map(|e| format!("{name}: {e}")).collect();
        }
    }
    problems.push(format!("`data` matches none of the reports `{command}` declares ({}): {}", outputs.iter().filter_map(|o| o["report"].as_str()).collect::<Vec<_>>().join(", "), best.join("; ")));
    problems
}

// ---- the state a leaf's example needs, shared by T9 and T12 ----

/// What one leaf's two runs happen in: the directory, the command line, the environment.
pub struct Case {
    pub dir: PathBuf,
    pub argv: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub _guards: Vec<TempDir>,
}

pub fn swapped(mut argv: Vec<String>, from: &str, to: &str) -> Vec<String> {
    let mut seen = false;
    for token in &mut argv {
        if token == from {
            *token = to.to_string();
            seen = true;
        }
    }
    assert!(seen, "the example has no `{from}`: {argv:?}");
    argv
}

pub fn put(repo: &Path, rel: &str, content: &str) {
    let target = repo.join(rel);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(target, content).unwrap();
}

pub const MANIFEST: &str = r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":[],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#;

pub const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";

pub const INVENTORY_YAML: &str = "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nauth.login.header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [auth.login.01]\n";

/// A complete brief with flow steps `F1` and `F2`.
pub fn clean_brief() -> String {
    "# Add login\n\n\
<!-- canon:section intent role=context -->\n## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
<!-- canon:section outcome role=normative -->\n## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
<!-- canon:section flow role=normative -->\n## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
<!-- canon:section detail role=normative -->\n## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
<!-- canon:section acceptance role=normative -->\n## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
<!-- canon:section questions role=question -->\n## Questions and assumptions\n\nNone.\n\n\
<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nSocial sign-in.\n\n"
        .to_string()
}

/// A strategy in the warm tier and the trajectories that corroborate it: what `strategy promote` re-renders.
pub fn seed_strategy(repo: &Path) -> String {
    use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
    use canon_learn::{ParquetStrategyStore, ParquetTrajectoryStore, StrategyId, StrategyItem, StrategyLifecycle, StrategyStore, Trajectory, TrajectoryId, TrajectoryStore, TrajectoryVerdict, VerdictOutcome};
    use canon_model::ids::{regime_key, RegimeKey, RoleId};
    use chrono::{Duration, Utc};

    let regime = RegimeKey::parse(regime_key("dev", "canon", "join-spine", "9c93d024b1a2")).unwrap();
    let role = RoleId::parse("dev").unwrap();
    let id = StrategyId::new();
    let item = StrategyItem::new(id, regime.clone(), role.clone(), "review guidance", "one-liner", "prefer the boring, correct option", vec![TrajectoryId::new()], Utc::now()).with_lifecycle(StrategyLifecycle::Active);
    ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies")).append(&item).expect("seed strategy");
    let store = ParquetTrajectoryStore::open(repo.join(".canon").join("learn").join("trajectories"));
    let now = Utc::now();
    for i in 0..5 {
        let row = VerdictRow { role: role.clone(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        let trajectory = Trajectory::new(TrajectoryId::new(), regime.clone(), "landed the change", "fixture", vec![row], now - Duration::days(5 - i), vec![]).expect("a well formed trajectory");
        store.append(&trajectory.with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.9))).expect("seed trajectory");
    }
    id.to_string()
}

/// The first `--manifest` value of an example.
pub fn manifest_of(argv: &[String]) -> String {
    let at = argv.iter().position(|a| a == "--manifest").expect("the example names a manifest");
    argv[at + 1].clone()
}

/// The directory and command line a leaf's idempotency check runs: its example in a copy of the
/// fixture, after the state the example needs is built (and the example's ids swapped for the ones built).
pub fn case(name: &str, base: &Path, home: &Path) -> Case {
    let node = leaf(name);
    let mut argv = example_argv(node);
    let mut envs: Vec<(String, String)> = Vec::new();
    let mut guards = Vec::new();
    let (guard, repo) = copy_of(base);
    guards.push(guard);
    let mut dir = repo.clone();
    match name {
        // A fresh directory: the second `init` finds what the first wrote.
        "init" => {
            let fresh = TempDir::new().unwrap();
            dir = fresh.path().to_path_buf();
            guards.push(fresh);
        }
        // The example's `auth.login.02` is in the fixture already.
        "scenario new" => argv = swapped(argv, "auth.login.02", "auth.login.03"),
        "review change" => {
            ok(&repo, home, &["change", "new", "tidy", "--subject-id", "demo", "--title", "Tidy", "--change-kind", "chore"]);
            argv = swapped(argv, "add-login", "tidy");
        }
        "context-pack add" | "prompt add" => put(&repo, &manifest_of(&argv), MANIFEST),
        "dispatch end" => {
            let mut begin = example_argv(leaf("dispatch begin"));
            begin.push("--json".into());
            let out = canon_in(&repo, home, &begin, &[]);
            assert!(out.status.success(), "dispatch begin: {}", String::from_utf8_lossy(&out.stdout));
            let run_id = document(&out)["data"]["runId"].as_str().expect("the run id").to_string();
            argv = swapped(argv, "01ARZ3NDEKTSV4RRFFQ69G5FAV", &run_id);
        }
        "brief approve" => {
            put(&repo, ".canon/changes/add-login/brief.md", &clean_brief());
            argv = swapped(argv, "add-search", "add-login");
        }
        "task done" => {
            put(&repo, ".canon/changes/add-login/brief.md", &clean_brief());
            put(&repo, ".canon/changes/add-login/tasks.md", "# Tasks: Add login\n\n- [ ] 1 Write the failure-path scenarios\n");
            ok(&repo, home, &["ingest", "plans"]);
            ok(&repo, home, &["brief", "approve", "add-login", "--approver", "alice", "--note", "approved as written"]);
            ok(&repo, home, &["evidence", "add", "--task-id", "add-login#1", "--class", "test-run", "--ref", "npm test", "--verdict", "faithful", "--summary", "1 passed", "--role", "implementer", "--actor-id", "impl"]);
        }
        "plugin sync" => {
            put(&repo, ".canon/plugins/porting/plugin.yaml", PORTING_YAML);
            put(&repo, ".canon/specs/inventory/kind=inventory/area=auth/surface=login/login.yaml", INVENTORY_YAML);
            ok(&repo, home, &["inventory", "sync"]);
            argv = swapped(argv, "my-plugin", "porting");
        }
        "strategy promote" => {
            let id = seed_strategy(&repo);
            argv = swapped(argv, "01ARZ3NDEKTSV4RRFFQ69G5FAV", &id);
        }
        // The example's `cold` tier: declared in canon.yaml, backed by a local directory through the debug seam the tier tests use.
        "artifacts move" => {
            let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
            let marker = "  # cold:\n  #   backend: s3\n  #   bucket_env: CANON_R2_BUCKET\n  #   prefix: \"canon/\"\n";
            assert!(yaml.contains(marker), "the skeleton's commented cold tier moved");
            std::fs::write(repo.join("canon.yaml"), yaml.replace(marker, "  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET\n    prefix: \"canon/\"\n")).unwrap();
            ok(&repo, home, &["inventory", "sync"]);
            let store = TempDir::new().unwrap();
            envs.push(("CANON_R2_LOCAL_ROOT".to_string(), store.path().display().to_string()));
            guards.push(store);
        }
        _ => {}
    }
    Case { dir, argv, envs, _guards: guards }
}

pub fn run_case(c: &Case, home: &Path) -> Output {
    let mut argv = c.argv.clone();
    argv.push("--json".into());
    let envs: Vec<(&str, &str)> = c.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    canon_in(&c.dir, home, &argv, &envs)
}

