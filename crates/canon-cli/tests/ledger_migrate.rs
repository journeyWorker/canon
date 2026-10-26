//! `canon migrate` and the 0.29 ledger layout (feature `ledger.migrate`,
//! scenarios 01-15; change `canon-0-30-ledger-naming`) through the real binary.
//!
//! Every test runs on its own copy of `tests/fixtures/ledger-0.29`: a repo tree
//! built by the 0.29 binary (project `root`, subject `demo`, changes `add-login`
//! and `tidy`, scenarios `auth.login.01..04`, 34 record files named by the 0.29
//! rule, three pre-0.23 reviews, a sqlite `.canon/hot.db`), stored without `.git`
//! and with its ignore file as `gitignore`. The copy gets a git repo and one commit.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_store::r2_tier::{ObjectRekey, R2Tier};
use canon_store::sqlite_tier::SqliteTier;
use canon_store::tier::RowRekey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// The fixture and the repo copies.

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger-0.29")
}

fn fixture_bytes(relative: &str) -> Vec<u8> {
    fs::read(fixture_dir().join(relative)).unwrap_or_else(|e| panic!("fixture file `{relative}`: {e}"))
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn files_under(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// The 0.29-named record files of the fixture, as repo-relative `/` paths: every `.json`
/// under a `kind=` directory of the ledger, its staging area and the archive.
fn fixture_record_files() -> Vec<String> {
    let root = fixture_dir();
    let mut all = Vec::new();
    files_under(&root.join(".canon/ledger"), &mut all);
    files_under(&root.join(".canon/archive"), &mut all);
    let mut out: Vec<String> = all
        .iter()
        .map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"))
        .filter(|p| p.ends_with(".json") && p.contains("/kind="))
        .collect();
    out.sort();
    out
}

/// The fixture's record files per kind directory of the committed ledger (`evidence_record`
/// spelled `evidence`): what `query --kind <k> --all` must count after the move.
fn fixture_ledger_counts() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for path in fixture_record_files() {
        let Some(rest) = path.strip_prefix(".canon/ledger/kind=") else { continue };
        let kind = rest.split('/').next().unwrap();
        let kind = if kind == "evidence_record" { "evidence" } else { kind };
        *counts.entry(kind.to_string()).or_insert(0) += 1;
    }
    counts
}

type Snapshot = BTreeMap<PathBuf, Vec<u8>>;

/// A repo copy: the work tree, the `HOME` every spawn gets, and an optional local object store.
struct Repo {
    dir: TempDir,
    home: TempDir,
    r2: Option<TempDir>,
}

impl Repo {
    /// The 0.29 fixture, in a git repo with one commit.
    fn old() -> Self {
        let dir = TempDir::new().unwrap();
        copy_tree(&fixture_dir(), dir.path());
        fs::rename(dir.path().join("gitignore"), dir.path().join(".gitignore")).unwrap();
        let repo = Self { dir, home: TempDir::new().unwrap(), r2: None };
        repo.git(&["init", "-q"]);
        repo.commit_all("fixture");
        repo
    }

    /// An empty git repo.
    fn empty() -> Self {
        let repo = Self { dir: TempDir::new().unwrap(), home: TempDir::new().unwrap(), r2: None };
        repo.git(&["init", "-q"]);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git_command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command
            .args(["-c", "gc.auto=0", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(self.path())
            .env("HOME", self.home.path())
            .env("GIT_AUTHOR_NAME", "ln-tests")
            .env("GIT_AUTHOR_EMAIL", "ln-tests@example.com")
            .env("GIT_COMMITTER_NAME", "ln-tests")
            .env("GIT_COMMITTER_EMAIL", "ln-tests@example.com");
        command
    }

    fn git(&self, args: &[&str]) -> String {
        let out = self.git_command(args).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn git_exit(&self, args: &[&str]) -> Option<i32> {
        self.git_command(args).output().expect("spawn git").status.code()
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
    }

    fn status(&self) -> Vec<String> {
        self.git(&["status", "--porcelain"]).lines().map(str::to_string).collect()
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
        command
            .args(args)
            .current_dir(self.path())
            .env_remove("CANON_ROLE")
            .env_remove("CANON_R2_LOCAL_ROOT")
            .env("CANON_ACTOR", "ln-tests")
            .env("HOME", self.home.path());
        if let Some(r2) = &self.r2 {
            command.env("CANON_R2_LOCAL_ROOT", r2.path());
        }
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("spawn canon binary")
    }

    /// `canon <args> --json`: the document on stdout and the process output.
    fn json(&self, args: &[&str]) -> (Value, Output) {
        let mut all = args.to_vec();
        all.push("--json");
        let out = self.run(&all);
        let doc = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("`canon {all:?}` printed no JSON document ({e}):\nstdout: {}\nstderr: {}", text(&out.stdout), text(&out.stderr)));
        (doc, out)
    }

    /// Every file under the repo (`.git` excluded), `.canon/migrate-backup` too unless `skip_backup`.
    fn snapshot(&self, skip_backup: bool) -> Snapshot {
        let mut files = Vec::new();
        files_under(self.path(), &mut files);
        files
            .into_iter()
            .map(|p| p.strip_prefix(self.path()).unwrap().to_path_buf())
            .filter(|p| !p.starts_with(".git") && !(skip_backup && p.starts_with(".canon/migrate-backup")))
            .map(|p| {
                let bytes = fs::read(self.path().join(&p)).unwrap();
                (p, bytes)
            })
            .collect()
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path().join(relative)).unwrap_or_else(|e| panic!("`{relative}`: {e}"))
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// The preview: the `canon migrate --json` document, which must be `ok` and write nothing itself.
    fn preview(&self) -> Value {
        let (doc, out) = self.json(&["migrate"]);
        assert!(out.status.success() && doc["ok"] == true, "the preview failed: {doc}");
        assert_eq!(doc["data"]["committed"], false, "a bare run must not commit: {doc}");
        doc
    }

    fn plan_digest(&self) -> String {
        self.preview()["data"]["planDigest"].as_str().expect("the preview prints a planDigest").to_string()
    }

    /// The preview, then `--commit --plan-digest` with its digest; returns the commit's document.
    fn migrate(&self) -> Value {
        let digest = self.plan_digest();
        let (doc, out) = self.json(&["migrate", "--commit", "--plan-digest", &digest]);
        assert!(out.status.success() && doc["ok"] == true && doc["data"]["committed"] == true, "the commit failed: {doc}\n{}", text(&out.stderr));
        doc
    }

    /// The repo gets a local object store (`CANON_R2_LOCAL_ROOT`) and a `cold` s3 tier in canon.yaml.
    fn with_object_store(mut self) -> Self {
        self.r2 = Some(TempDir::new().unwrap());
        let yaml = self.read("canon.yaml");
        let marker = "    root: .canon/ledger\n";
        assert!(yaml.contains(marker), "the fixture's canon.yaml changed");
        let yaml = yaml.replacen(marker, &format!("{marker}  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET_TEST_UNUSED\n    prefix: \"canon/\"\n"), 1);
        self.write("canon.yaml", &yaml);
        self.commit_all("cold tier");
        self
    }

    fn object_store(&self) -> R2Tier {
        R2Tier::local(self.r2.as_ref().expect("a repo with an object store").path(), "canon/").unwrap()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn combined(out: &Output) -> String {
    format!("{}{}", text(&out.stdout), text(&out.stderr))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The record's canonical form: compact, object keys sorted at every depth.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let members: Vec<String> = keys.into_iter().map(|k| format!("{}:{}", serde_json::to_string(k).unwrap(), canonical(&map[k]))).collect();
            format!("{{{}}}", members.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(canonical).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

/// The 0.30 digest of a record body: the first 8 hex of the sha256 of its canonical form.
fn digest8(body: &Value) -> String {
    sha256_hex(canonical(body).as_bytes())[..8].to_string()
}

fn changed(before: &Snapshot, after: &Snapshot) -> Vec<PathBuf> {
    before.keys().chain(after.keys()).collect::<BTreeSet<_>>().into_iter().filter(|p| before.get(*p) != after.get(*p)).cloned().collect()
}

fn is_sqlite_sidecar(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == "hot.db-wal" || n == "hot.db-shm")
}

/// The tree is what `before` says, the sqlite file included. The `-wal`/`-shm` sidecars of the
/// sqlite file are left out here; `ledger_migrate_04b` pins them on their own.
fn assert_unchanged(before: &Snapshot, repo: &Repo, what: &str) {
    let after = repo.snapshot(false);
    let paths: Vec<PathBuf> = changed(before, &after).into_iter().filter(|p| !is_sqlite_sidecar(p)).collect();
    assert!(paths.is_empty(), "{what} changed the tree: {paths:?}");
}

/// Strict: every file, the sqlite sidecars too.
fn assert_unchanged_strictly(before: &Snapshot, repo: &Repo, what: &str) {
    let paths = changed(before, &repo.snapshot(false));
    assert!(paths.is_empty(), "{what} changed the tree: {paths:?}");
}

fn warnings(doc: &Value) -> Vec<String> {
    doc["meta"]["warnings"].as_array().map(|a| a.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn steps(doc: &Value) -> Vec<String> {
    doc["data"]["steps"].as_array().expect("steps").iter().map(|s| s.as_str().expect("a step is a string").to_string()).collect()
}

fn effects(doc: &Value) -> Vec<&Value> {
    doc["data"]["effects"].as_array().expect("effects").iter().collect()
}

fn effects_of<'a>(doc: &'a Value, action: &str) -> Vec<&'a Value> {
    effects(doc).into_iter().filter(|e| e["action"] == action).collect()
}

fn violations(doc: &Value) -> Vec<(String, String)> {
    doc["data"]["violations"].as_array().expect("violations").iter().map(|v| (v["class"].as_str().unwrap().to_string(), v["subject"].as_str().unwrap().to_string())).collect()
}

fn advisory_kinds(doc: &Value) -> Vec<String> {
    doc["data"]["advisories"].as_array().map(|a| a.iter().filter_map(|v| v["kind"].as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// A write the 0.29 layout refuses: not `ok`, not a success exit, code `OLD_LAYOUT`, the hint names `canon migrate`.
fn assert_old_layout_refusal(repo: &Repo, args: &[&str]) {
    let (doc, out) = repo.json(args);
    assert!(!out.status.success(), "`canon {args:?}` exited 0 in a 0.29 repo: {doc}");
    assert_eq!(doc["ok"], false, "`canon {args:?}`: {doc}");
    assert_eq!(doc["error"]["code"], "OLD_LAYOUT", "`canon {args:?}`: {doc}");
    let hint = doc["error"]["hint"].as_str().unwrap_or_default();
    assert!(hint.contains("canon migrate"), "`canon {args:?}` hint does not name `canon migrate`: {hint}");
}

const EVIDENCE_ADD: [&str; 16] = [
    "evidence",
    "add",
    "--scenario-id",
    "auth.login.01",
    "--project-id",
    "root",
    "--actor-id",
    "a",
    "--role",
    "implementer",
    "--class",
    "test-run",
    "--ref",
    "r",
    "--verdict",
    "faithful",
];

fn evidence_add(repo: &Repo) -> (Value, Output) {
    let mut args = EVIDENCE_ADD.to_vec();
    args.extend(["--summary", "s", "--override-reason", "x"]);
    repo.json(&args)
}

/// The migrated review file (by scenario id) as a JSON value, and its path.
fn review_of(repo: &Repo, scenario: &str) -> (PathBuf, Value) {
    let mut files = Vec::new();
    files_under(&repo.path().join(".canon/ledger/kind=review"), &mut files);
    let mut found: Vec<(PathBuf, Value)> = files
        .into_iter()
        .filter_map(|p| {
            let value: Value = serde_json::from_slice(&fs::read(&p).ok()?).ok()?;
            (value["scenario_id"] == scenario).then_some((p, value))
        })
        .collect();
    assert_eq!(found.len(), 1, "exactly one review file of {scenario}: {found:?}");
    found.remove(0)
}

fn evidence_files(repo: &Repo, root: &str) -> Vec<(String, Value)> {
    let mut files = Vec::new();
    files_under(&repo.path().join(root).join("kind=evidence"), &mut files);
    files.sort();
    files.into_iter().map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), serde_json::from_slice(&fs::read(&p).unwrap()).unwrap())).collect()
}

// ---------------------------------------------------------------------------
// ledger.migrate.01

/// ledger.migrate.01 -- Then: `canon status`, `canon gate check` and `canon context` each
/// succeed in reading with no config error and warn that the layout is 0.29; `canon migrate
/// --json` is the first next step; no old-named file counts toward any verdict.
#[test]
fn ledger_migrate_01_read_commands_warn_and_point_at_migrate() {
    let repo = Repo::old();
    for (args, exit) in [(vec!["status"], 0), (vec!["gate", "check"], 1), (vec!["context"], 0)] {
        let (doc, out) = repo.json(&args);
        assert_eq!(out.status.code(), Some(exit), "`canon {args:?}` exit: {doc}");
        if let Some(code) = doc["error"]["code"].as_str() {
            assert!(!code.starts_with("CONFIG"), "`canon {args:?}` fails on configuration: {doc}");
            assert_eq!(code, "GATE_VIOLATIONS", "`canon {args:?}` fails with {code}: {doc}");
        } else {
            assert_eq!(doc["ok"], true, "`canon {args:?}`: {doc}");
        }
        let layout_warning = warnings(&doc).into_iter().any(|w| w.contains("0.29") && w.contains("canon migrate"));
        assert!(layout_warning, "`canon {args:?}` does not warn that the layout is 0.29 and name `canon migrate`: {:?}", warnings(&doc));
        assert_eq!(doc["meta"]["next"][0]["command"], "canon migrate --json", "`canon {args:?}`: first next step is {}", doc["meta"]["next"][0]);
    }
    // The human forms warn too, and none of them fails on configuration.
    for args in [["status"].as_slice(), ["gate", "check"].as_slice(), ["context"].as_slice()] {
        let out = repo.run(args);
        let all = combined(&out);
        assert!(all.contains("0.29") && all.contains("canon migrate"), "`canon {args:?}` does not warn: {all}");
        assert!(!all.contains("CONFIG"), "`canon {args:?}` fails on configuration: {all}");
    }
}

/// ledger.migrate.01 -- Then: no old-named file counts toward any verdict (the gate sees no
/// evidence, change or subject; every scenario is uncovered; status sees no subject).
#[test]
fn ledger_migrate_01b_no_old_named_file_counts_toward_a_verdict() {
    let repo = Repo::old();
    let (gate, _) = repo.json(&["gate", "check"]);
    assert_eq!(gate["meta"]["coverage"]["evidence"], 0, "{}", gate["meta"]["coverage"]);
    assert_eq!(gate["meta"]["coverage"]["changes"], 0, "{}", gate["meta"]["coverage"]);
    assert_eq!(gate["meta"]["coverage"]["subjects"], 0, "{}", gate["meta"]["coverage"]);
    let found = violations(&gate);
    for scenario in ["auth.login.01", "auth.login.02", "auth.login.03", "auth.login.04"] {
        assert!(found.contains(&("uncovered-cell".to_string(), scenario.to_string())), "{scenario} counts evidence from an old-named file: {found:?}");
    }
    let old: Vec<_> = found.iter().filter(|(class, _)| class == "old-layout").collect();
    assert!(!old.is_empty(), "the old-named files are not reported: {found:?}");
    let advisories = advisory_kinds(&gate);
    assert!(!advisories.contains(&"brief_overrides".to_string()) && !advisories.contains(&"unmigrated_review".to_string()), "an old-named evidence or review file is read: {advisories:?}");

    let (status, _) = repo.json(&["status"]);
    assert_eq!(status["data"]["subjects"].as_array().map(Vec::len), Some(0), "status counts an old-named subject file: {}", status["data"]["subjects"]);
}

// ---------------------------------------------------------------------------
// ledger.migrate.02

/// ledger.migrate.02 -- Then: `canon evidence add` is refused OLD_LAYOUT with the `canon migrate`
/// hint and the tree is byte-identical afterwards; no file in the new name appears beside the old ones.
#[test]
fn ledger_migrate_02_evidence_add_is_refused_and_writes_nothing() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    assert_old_layout_refusal(&repo, &EVIDENCE_ADD.iter().copied().chain(["--summary", "s", "--override-reason", "x"]).collect::<Vec<_>>());
    assert_unchanged(&before, &repo, "the refused `canon evidence add`");
    let after = repo.snapshot(false);
    assert!(after.keys().all(|p| !p.to_string_lossy().contains("kind=evidence/")), "a file in the new name appeared: {:?}", after.keys().filter(|p| p.to_string_lossy().contains("kind=evidence/")).collect::<Vec<_>>());
}

/// ledger.migrate.02 -- Then: refused with the same hint in the human form too.
#[test]
fn ledger_migrate_02b_the_human_refusal_names_the_code_and_the_hint() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    let mut args = EVIDENCE_ADD.to_vec();
    args.extend(["--summary", "s", "--override-reason", "x"]);
    let out = repo.run(&args);
    assert!(!out.status.success());
    let all = combined(&out);
    assert!(all.contains("OLD_LAYOUT") && all.contains("canon migrate"), "{all}");
    assert_unchanged(&before, &repo, "the refused `canon evidence add`");
}

// ---------------------------------------------------------------------------
// ledger.migrate.03

/// ledger.migrate.03 -- Then: in a 0.30 ledger with one 0.29-named file, `canon gate check`
/// reports exactly that file as `old-layout` with the hint `canon migrate`; the 0.30 records
/// are not reported and the old file counts toward no verdict.
#[test]
fn ledger_migrate_03_one_old_named_file_is_reported_alone() {
    let repo = Repo::old();
    repo.migrate();
    repo.commit_all("migrated");
    let (clean, _) = repo.json(&["gate", "check"]);
    let baseline = violations(&clean);
    assert!(baseline.iter().all(|(class, _)| class != "old-layout"), "the migrated repo still reports old-layout files: {baseline:?}");
    let findings_before = repo.json(&["query", "--kind", "finding", "--all"]).0["data"]["count"].clone();

    // One finding, copied under its 0.29 name, with a body that differs from the migrated one.
    let dir = repo.path().join(".canon/ledger/kind=finding");
    let source = fs::read_dir(&dir).unwrap().flatten().find(|e| e.file_name().to_string_lossy().starts_with("add-login_2_3_")).expect("the migrated finding of round 2").path();
    let mut body: Value = serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
    body["seq"] = json!(4);
    body["summary"] = json!("a finding only the 0.29 name holds");
    let old_name = "kind=finding/add-login__0002__0004__abcdef012345.json";
    repo.write(&format!(".canon/ledger/{old_name}"), &serde_json::to_string_pretty(&body).unwrap());

    let (gate, _) = repo.json(&["gate", "check"]);
    let found = violations(&gate);
    let old: Vec<_> = found.iter().filter(|(class, _)| class == "old-layout").collect();
    assert_eq!(old.len(), 1, "exactly one old-layout violation expected: {found:?}");
    assert_eq!(old[0].1, old_name, "the violation names another file: {found:?}");
    let violation = gate["data"]["violations"].as_array().unwrap().iter().find(|v| v["class"] == "old-layout").unwrap();
    assert!(violation["message"].as_str().unwrap().contains("canon migrate"), "the hint is missing: {violation}");
    let rest: Vec<_> = found.iter().filter(|(class, _)| class != "old-layout").cloned().collect();
    assert_eq!(rest, baseline, "the 0.30 records are reported, or a verdict moved");
    assert_eq!(gate["meta"]["coverage"], clean["meta"]["coverage"], "the old file counts toward the coverage");
    let findings_after = repo.json(&["query", "--kind", "finding", "--all"]).0["data"]["count"].clone();
    assert_eq!(findings_after, findings_before, "the old-named finding is counted");
}

// ---------------------------------------------------------------------------
// ledger.migrate.04

/// ledger.migrate.04 -- Then: the bare preview lists the files to move (ledger, archive,
/// staging, overlay), the evidence bodies to rewrite with old key, digest and body hash, the
/// canon.yaml keys, the gitignore lines and the sqlite rows, with a plan digest; the working
/// tree, the sqlite file and canon.yaml are byte-identical afterwards.
#[test]
fn ledger_migrate_04_preview_lists_everything_and_writes_nothing() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    let doc = repo.preview();
    assert_unchanged(&before, &repo, "the preview");

    let digest = doc["data"]["planDigest"].as_str().unwrap();
    assert!(digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()), "planDigest: {digest}");
    assert_eq!(repo.plan_digest(), digest, "the plan digest is not stable between two previews");

    // Every 0.29-named record file moves, and the evidence ones say what they were.
    let moves: BTreeMap<String, String> = effects_of(&doc, "move").into_iter().map(|e| (e["path"].as_str().unwrap().to_string(), e["detail"].as_str().unwrap_or_default().to_string())).collect();
    let files = fixture_record_files();
    assert_eq!(files.len(), 34, "the fixture changed: {files:?}");
    for file in &files {
        assert!(moves.contains_key(file), "the preview does not list `{file}`; it lists {:?}", moves.keys().collect::<Vec<_>>());
    }
    for (kind, label) in [("kind=evidence_record", "evidence"), ("_staging/kind=divergence", "staged"), ("kind=porting.coverage", "overlay"), ("archive/kind=", "archive")] {
        assert!(moves.keys().any(|p| p.contains(kind)), "no {label} file in the preview");
    }
    for file in files.iter().filter(|f| f.contains("/kind=evidence_record/")) {
        let stem = Path::new(file).file_stem().unwrap().to_string_lossy().into_owned();
        let (key, digest12) = stem.split_once("__").unwrap();
        let hash = sha256_hex(&fixture_bytes(file));
        let want = format!("was key {key}, digest {digest12}, body sha256:{hash}");
        assert!(moves[file].contains(&want), "`{file}`: expected `{want}` in `{}`", moves[file]);
        assert!(moves[file].contains("`evidence`"), "`{file}`: the kind value change is not said: {}", moves[file]);
    }

    // The sqlite rows, canon.yaml, the gitignore.
    let rows: Vec<String> = effects_of(&doc, "rekey").into_iter().map(|e| e["record"].as_str().unwrap().to_string()).collect();
    assert_eq!(rows.len(), 4, "{rows:?}");
    for prefix in ["sqlite:hot.db:evidence_record:auth.login.04:", "sqlite:hot.db:run:01M5WJNYSXW7ZM26Q8ZJTF70C1:", "sqlite:hot.db:task:add-login#1:", "sqlite:hot.db:task:add-login#2:"] {
        assert!(rows.iter().any(|r| r.starts_with(prefix)), "no row `{prefix}…` in {rows:?}");
    }
    let rewrites: Vec<String> = effects_of(&doc, "rewrite").into_iter().map(|e| e["path"].as_str().unwrap_or_default().to_string()).collect();
    assert!(rewrites.contains(&"canon.yaml".to_string()) && rewrites.contains(&".gitignore".to_string()), "{rewrites:?}");
    let all_steps = steps(&doc).join("\n");
    assert!(all_steps.contains("routing.evidence_record"), "the canon.yaml key is not named: {all_steps}");
    assert!(all_steps.contains("kind=evidence_record") && all_steps.contains("kind=evidence`"), "the gitignore line is not named: {all_steps}");
    let ignore = effects_of(&doc, "rewrite").into_iter().find(|e| e["path"] == ".gitignore" && e["detail"].as_str().is_some_and(|d| d.contains("kind=evidence_record"))).expect("a .gitignore effect for the negation");
    assert!(ignore["detail"].as_str().unwrap().contains("kind=evidence`"), "{ignore}");

    // The human form ends with the digest and the way to act.
    let human = repo.run(&["migrate"]);
    assert!(human.status.success());
    let out = text(&human.stdout);
    let last = out.trim_end().lines().last().unwrap();
    assert_eq!(last, format!("planDigest {digest}; nothing written. To act: canon migrate --commit --plan-digest {digest}"), "{out}");
    assert_unchanged(&before, &repo, "the human preview");
}

/// ledger.migrate.04 -- Then: the working tree is byte-identical afterwards, with no file
/// added beside the sqlite file (its `-wal`/`-shm` sidecars) by the preview or by a refused run.
#[test]
fn ledger_migrate_04b_preview_and_refusals_leave_no_sqlite_sidecar_files() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    repo.preview();
    assert_unchanged_strictly(&before, &repo, "the preview");
    let (doc, out) = repo.json(&["migrate", "--commit", "--plan-digest", &"a".repeat(64)]);
    assert!(!out.status.success(), "{doc}");
    assert_unchanged_strictly(&before, &repo, "the refused commit");
}

// ---------------------------------------------------------------------------
// ledger.migrate.05

fn assert_refused_naming(repo: &Repo, file: &str) {
    let before = repo.snapshot(false);
    let (doc, out) = repo.json(&["migrate"]);
    assert!(!out.status.success() && doc["ok"] == false, "the preview is not refused: {doc}");
    assert!(doc["error"]["message"].as_str().unwrap().contains(file), "the refusal does not name `{file}`: {doc}");
    assert_unchanged(&before, repo, "the refused preview");
    let (doc, out) = repo.json(&["migrate", "--commit", "--plan-digest", &"a".repeat(64)]);
    assert!(!out.status.success() && doc["ok"] == false, "the commit is not refused: {doc}");
    assert!(doc["error"]["message"].as_str().unwrap().contains(file), "the commit's refusal does not name `{file}`: {doc}");
    assert_unchanged(&before, repo, "the refused commit");
}

/// ledger.migrate.05 -- Then: a staged body that a committed record names as its `staging_id`
/// (an interrupted promote) is refused naming the staged file, and nothing is written.
#[test]
fn ledger_migrate_05_an_interrupted_promote_in_staging_refuses_the_plan() {
    let repo = Repo::old();
    let staged = ".canon/ledger/_staging/kind=divergence/area=auth/root__auth.login.02__0000000000__000002__f4af29d00335.json";
    let committed = ".canon/ledger/kind=divergence/area=auth/root__auth.login.01__0000000001__000001__f6895287e78a.json";
    // The staged file's name ends in the digest of its body; the committed divergence now names it.
    let mut body: Value = serde_json::from_str(&repo.read(committed)).unwrap();
    body["staging_id"] = json!("f4af29d00335");
    repo.write(committed, &format!("{}\n", serde_json::to_string_pretty(&body).unwrap()));
    repo.commit_all("a promote that stopped halfway");
    assert_refused_naming(&repo, staged);
}

/// ledger.migrate.05 -- Then: an untracked file under a moved directory refuses the plan,
/// naming it, and nothing is written.
#[test]
fn ledger_migrate_05b_an_untracked_file_under_a_moved_directory_refuses_the_plan() {
    let repo = Repo::old();
    repo.write(".canon/ledger/kind=finding/stray.json", "{}\n");
    assert_refused_naming(&repo, ".canon/ledger/kind=finding/stray.json");
}

/// ledger.migrate.05 -- Then: a modified tracked file under a moved directory refuses the plan,
/// naming it, and nothing is written.
#[test]
fn ledger_migrate_05c_a_modified_tracked_file_under_a_moved_directory_refuses_the_plan() {
    let repo = Repo::old();
    let file = ".canon/ledger/kind=finding/add-login__0010__0001__52fa6e3ae5ae.json";
    let edited = format!("{}\n", repo.read(file));
    repo.write(file, &edited);
    assert_refused_naming(&repo, file);
}

// ---------------------------------------------------------------------------
// ledger.migrate.06

/// ledger.migrate.06 -- Then: committing the previewed plan moves the files with git, evidence
/// bodies carry `kind: evidence` and their recomputed digests, canon.yaml says `evidence` and the
/// gitignore negation follows.
#[test]
fn ledger_migrate_06_commit_renames_with_git_and_rewrites_evidence_canon_yaml_and_gitignore() {
    let repo = Repo::old();
    let preview = repo.preview();
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let tracked: BTreeSet<String> = repo.git(&["ls-files"]).lines().map(str::to_string).collect();
    let (doc, out) = repo.json(&["migrate", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && doc["ok"] == true, "{doc}");
    assert_eq!(doc["data"]["planDigest"], digest.as_str());

    // Moved with git: every planned move of a tracked file is a rename in the index; an ignored file just moves.
    let status = repo.status();
    for effect in effects_of(&preview, "move") {
        let (from, to) = (effect["path"].as_str().unwrap(), effect["detail"].as_str().unwrap().strip_prefix("to ").unwrap().split(';').next().unwrap());
        assert!(!repo.path().join(from).exists(), "`{from}` is still there");
        if tracked.contains(from) {
            let line = status.iter().find(|l| l.contains(&format!("{from} -> {to}")));
            assert!(line.is_some_and(|l| l.starts_with('R')), "`{from}` -> `{to}` is no git rename; status:\n{}", status.join("\n"));
        } else if !from.contains("kind=scenario/") {
            // A derived scenario record is skipped: the index refresh the migrate runs first writes the 0.30 steps onto it, which supersedes it (0.30 `record-links`).
            assert!(repo.path().join(to).is_file(), "the untracked `{from}` did not arrive at `{to}`");
        }
    }
    assert!(!repo.path().join(".canon/ledger/kind=evidence_record").exists() || fs::read_dir(repo.path().join(".canon/ledger/kind=evidence_record")).unwrap().next().is_none(), "files are left under kind=evidence_record");

    // Evidence bodies: kind `evidence`, and the 8-hex digest in the name is the body's.
    for root in [".canon/ledger", ".canon/archive"] {
        let found = evidence_files(&repo, root);
        assert_eq!(found.len(), if root == ".canon/ledger" { 6 } else { 2 }, "{root}: {found:?}");
        for (name, body) in found {
            assert_eq!(body["kind"], "evidence", "{root}/{name}");
            let stem = name.strip_suffix(".json").unwrap();
            assert_eq!(stem.rsplit_once('_').unwrap().1, digest8(&body), "{root}/{name}: the digest in the name is not the body's");
            assert!(!stem.contains("__"), "{root}/{name}: still named by the 0.29 rule");
        }
    }
    // canon.yaml and the gitignore.
    let yaml = repo.read("canon.yaml");
    assert!(yaml.lines().any(|l| l.trim() == "evidence: local") && !yaml.contains("evidence_record"), "canon.yaml:\n{yaml}");
    let ignore = repo.read(".gitignore");
    assert!(ignore.lines().any(|l| l == "!.canon/ledger/kind=evidence/") && !ignore.contains("evidence_record"), ".gitignore:\n{ignore}");
}

/// ledger.migrate.06 -- Then: gate check, status and query give the same judgements and records
/// as before, apart from names, the kind name and the order of records sharing an `at`.
#[test]
fn ledger_migrate_06b_judgements_and_records_are_what_0_29_gave() {
    let repo = Repo::old();
    repo.migrate();

    let (gate, _) = repo.json(&["gate", "check"]);
    let found: BTreeSet<_> = violations(&gate).into_iter().collect();
    let expected: BTreeSet<_> = [("uncovered-cell", "auth.login.01"), ("uncovered-cell", "auth.login")].into_iter().map(|(c, s)| (c.to_string(), s.to_string())).collect();
    assert_eq!(found, expected, "the 0.29 gate said exactly these");
    let open_divergence = gate["data"]["violations"].as_array().unwrap().iter().find(|v| v["subject"] == "auth.login.01").unwrap();
    assert!(open_divergence["message"].as_str().unwrap().contains("open divergence"), "{open_divergence}");
    let coverage = &gate["meta"]["coverage"];
    assert_eq!((coverage["changes"].clone(), coverage["evidence"].clone(), coverage["scenarios"].clone(), coverage["subjects"].clone()), (json!(2), json!(6), json!(4), json!(1)), "{coverage}");
    assert!(warnings(&gate).iter().all(|w| !w.contains("0.29")), "the migrated repo still warns: {:?}", warnings(&gate));

    let (status, out) = repo.json(&["status"]);
    assert!(out.status.success(), "{status}");
    let subjects = status["data"]["subjects"].as_array().unwrap();
    assert_eq!(subjects.len(), 1, "{subjects:?}");
    assert_eq!((subjects[0]["id"].as_str(), subjects[0]["status"].as_str()), (Some("demo"), Some("proposed")));
    let unowned: Vec<&str> = status["data"]["unowned"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
    assert_eq!(unowned, ["auth.login.01", "auth.login.02", "auth.login.03", "auth.login.04"]);

    // `query` counts what the fixture's files per kind say.
    for (kind, count) in fixture_ledger_counts() {
        if kind == "porting.coverage" {
            continue; // an overlay kind: no `query --kind`
        }
        let (query, out) = repo.json(&["query", "--kind", &kind, "--all"]);
        assert!(out.status.success(), "query {kind}: {query}");
        assert_eq!(query["data"]["count"], count, "query --kind {kind}");
        assert_eq!(query["data"]["records"].as_array().map(Vec::len), Some(count), "query --kind {kind}");
    }
    // The order, kind by kind, against what 0.29 gave: the kinds it folded (key-ordered map) by their joined key, the
    // others (evidence, review, divergence, ...) by `at`. The fixture's `at` order differs from its name order for evidence
    // and review, and `at` ties only among scenarios (key-ordered), so the two orders cannot be confused.
    const KEY_ORDERED: [&str; 4] = ["change", "finding", "scenario", "subject"];
    let identity = |v: &Value| -> String {
        [
            "subject_id",
            "change_id",
            "scenario_id",
            "staging_id",
            "round",
            "seq",
        ]
        .iter()
        .filter_map(|f| v.get(*f))
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("/")
    };
    for kind in fixture_ledger_counts()
        .into_keys()
        .filter(|k| k != "porting.coverage")
    {
        let dir = if kind == "evidence" {
            "evidence_record"
        } else {
            kind.as_str()
        };
        let mut files: Vec<(String, chrono::DateTime<chrono::Utc>, String)> =
            fixture_record_files()
                .into_iter()
                .filter_map(|p| {
                    p.strip_prefix(&format!(".canon/ledger/kind={dir}/"))
                        .map(str::to_string)
                })
                .map(|name| {
                    let body: Value = serde_json::from_slice(&fixture_bytes(&format!(
                        ".canon/ledger/kind={dir}/{name}"
                    )))
                    .unwrap();
                    let at = body
                        .get("at")
                        .or_else(|| body["envelope"].get("at"))
                        .and_then(Value::as_str)
                        .unwrap()
                        .parse()
                        .unwrap();
                    let file = name.rsplit('/').next().unwrap();
                    let key = file.rsplit_once("__").unwrap().0.to_string();
                    (key, at, identity(&body))
                })
                .collect();
        if KEY_ORDERED.contains(&kind.as_str()) {
            files.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
        } else {
            files.sort_by_key(|f| f.1);
        }
        let expected: Vec<String> = files.into_iter().map(|f| f.2).collect();
        let (query, _) = repo.json(&["query", "--kind", &kind, "--all"]);
        let got: Vec<String> = query["data"]["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(&identity)
            .collect();
        assert_eq!(
            got, expected,
            "`query --kind {kind}` order differs from 0.29's"
        );
    }
    let (evidence, _) = repo.json(&["query", "--kind", "evidence", "--all"]);
    let kinds: BTreeSet<&str> = evidence["data"]["records"].as_array().unwrap().iter().map(|r| r["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, BTreeSet::from(["evidence"]));
}

/// ledger.migrate.06 -- Then: the second run reports nothing to move.
#[test]
fn ledger_migrate_06c_a_second_run_has_nothing_to_move() {
    let repo = Repo::old();
    repo.migrate();
    let (doc, out) = repo.json(&["migrate"]);
    assert!(out.status.success() && doc["ok"] == true, "{doc}");
    assert!(effects_of(&doc, "move").is_empty(), "the second preview still moves files: {:?}", effects_of(&doc, "move"));
    assert!(effects_of(&doc, "rekey").is_empty(), "the second preview still re-keys rows");
    let human = repo.run(&["migrate"]);
    assert!(combined(&human).contains("nothing to migrate"), "{}", combined(&human));
}

// ---------------------------------------------------------------------------
// ledger.migrate.07 and .13

fn pin_evidence_of_file(path: &str) -> Value {
    let value: Value = serde_json::from_slice(&fixture_bytes(path)).unwrap();
    value["pins"]["evidence"].clone()
}

/// ledger.migrate.07 -- Then: pre-0.23 reviews that pin evidence by 0.29 key and digest are
/// judged from the plan's record of the old world: the current one (02) is rewritten with 0.23
/// pins and counted; the stale one (03) stays and is still reported; the outcome line says
/// "1 were pinned to evidence records".
#[test]
fn ledger_migrate_07_pre_0_23_reviews_keep_their_standing() {
    let repo = Repo::old();
    // The fixture: 02 pins the digest of its latest evidence, 03 an older one.
    assert_eq!(pin_evidence_of_file(".canon/ledger/kind=review/area=auth/root__auth.login.02__1111111111111111111111111111111111111111__1c4b26b9031c.json"), json!([{"key": "auth.login.02", "digest": "f778690a0216"}]));
    assert_eq!(pin_evidence_of_file(".canon/ledger/kind=review/area=auth/root__auth.login.03__2222222222222222222222222222222222222222__d5d475c40d64.json"), json!([{"key": "auth.login.03", "digest": "0d75c144dd76"}]));

    let preview = repo.preview();
    assert!(steps(&preview).iter().any(|s| s.contains("would pin 2 review(s)") && s.contains("1 were pinned to evidence records")), "{:#?}", steps(&preview));
    let done = repo.migrate();
    assert!(steps(&done).iter().any(|s| s.contains("pinned 2 review(s)") && s.contains("1 were pinned to evidence records")), "{:#?}", steps(&done));

    // 02 (current): pinned to what its evidence says, no longer by key and digest.
    let (_, current) = review_of(&repo, "auth.login.02");
    assert!(current["pins"]["checks"].is_array(), "review 02 has no 0.23 pins: {current}");
    assert!(current["pins"].get("evidence").is_none(), "review 02 still pins by key and digest: {current}");
    // 03 (stale): left as it was, and the gate still reports it, alone.
    let (path, stale) = review_of(&repo, "auth.login.03");
    assert_eq!(fs::read(&path).unwrap(), fixture_bytes(".canon/ledger/kind=review/area=auth/root__auth.login.03__2222222222222222222222222222222222222222__d5d475c40d64.json"), "review 03 was rewritten");
    assert_eq!(
        stale["pins"]["evidence"],
        json!([{"key": "auth.login.03", "digest": "0d75c144dd76"}])
    );
    let (gate, _) = repo.json(&["gate", "check"]);
    let advisory = gate["data"]["advisories"].as_array().unwrap().iter().find(|a| a["kind"] == "unmigrated_review").expect("the stale review is still reported");
    let message = advisory["message"].as_str().unwrap();
    assert!(message.contains("auth.login.03") && !message.contains("auth.login.02") && !message.contains("auth.login.04") && message.starts_with("1 scenario(s)"), "{message}");
    // The 0.23-pinned and the change reviews are untouched.
    let (path, _) = review_of(&repo, "auth.login.01");
    assert_eq!(fs::read(path).unwrap(), fixture_bytes(".canon/ledger/kind=review/area=auth/root__auth.login.01__78c4ae0b565453a98ac671ab231ac18ea07db208__83a30eec3002.json"));
}

/// ledger.migrate.13 -- Then: the stale pre-0.23 review is left as it is and still stale, and
/// the one that cannot be checked (its commit is not in the clone) is pinned as it stands and counted.
#[test]
fn ledger_migrate_13_stale_stays_stale_and_the_undecidable_one_is_pinned_and_counted() {
    let repo = Repo::old();
    let preview = repo.preview();
    let done = repo.migrate();
    for (doc, verb) in [(&preview, "would pin"), (&done, "pinned")] {
        let all = steps(doc);
        assert!(all.iter().any(|s| s.contains(verb) && s.contains("1 of those could not be checked") && s.contains("pinned as the scenario stands")), "{verb}: {all:#?}");
        assert!(all.iter().any(|s| s.contains("left 1 stale review(s) as they are") && s.contains("stays stale until it is reviewed again")), "{verb}: {all:#?}");
    }
    // 04: pre-0.15, no pins at all, its commit is not here: pinned as it stands.
    let old_04: Value = serde_json::from_slice(&fixture_bytes(".canon/ledger/kind=review/area=auth/root__auth.login.04__3333333333333333333333333333333333333333__ed582aaa9491.json")).unwrap();
    assert!(old_04.get("pins").is_none(), "the fixture's review 04 has pins");
    let (_, pinned) = review_of(&repo, "auth.login.04");
    assert!(pinned["pins"]["checks"].is_array() && pinned["pins"]["scenario_digest"].is_string(), "review 04 is not pinned: {pinned}");
    // 03: byte-identical to the fixture's, and the gate says it is stale (the advisory names it alone).
    let (path, _) = review_of(&repo, "auth.login.03");
    assert_eq!(fs::read(path).unwrap(), fixture_bytes(".canon/ledger/kind=review/area=auth/root__auth.login.03__2222222222222222222222222222222222222222__d5d475c40d64.json"));
    let (gate, _) = repo.json(&["gate", "check"]);
    let advisory = gate["data"]["advisories"].as_array().unwrap().iter().find(|a| a["kind"] == "unmigrated_review").expect("the stale review is reported");
    assert!(advisory["message"].as_str().unwrap().contains("(auth.login.03)"), "{advisory}");
}

// ---------------------------------------------------------------------------
// ledger.migrate.08

/// ledger.migrate.08 -- Then: `migrate --commit` with a digest that is not the preview's is
/// refused PLAN_CHANGED and the tree, the sqlite file and the config are unchanged.
#[test]
fn ledger_migrate_08_another_digest_is_refused_plan_changed() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    for wrong in ["a".repeat(64), "0".repeat(64)] {
        let (doc, out) = repo.json(&["migrate", "--commit", "--plan-digest", &wrong]);
        assert!(!out.status.success() && doc["ok"] == false, "{doc}");
        assert_eq!(doc["error"]["code"], "PLAN_CHANGED", "{doc}");
        assert_unchanged(&before, &repo, "the refused commit");
    }
    assert_eq!(repo.snapshot(false).get(Path::new(".canon/hot.db")), before.get(Path::new(".canon/hot.db")));
    assert_eq!(repo.snapshot(false).get(Path::new("canon.yaml")), before.get(Path::new("canon.yaml")));
    assert!(repo.status().is_empty(), "git status: {:?}", repo.status());
}

// ---------------------------------------------------------------------------
// ledger.migrate.09

/// ledger.migrate.09 -- Then: after the move is reverted from its backup the tree and the sqlite
/// file are byte-identical to before the migration, and the git index is restored.
#[test]
fn ledger_migrate_09_revert_restores_the_tree_the_sqlite_file_and_the_index() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    repo.migrate();
    assert_ne!(repo.snapshot(true), before, "the migration changed nothing");

    // The preview writes nothing; the revert acts on its digest.
    let migrated = repo.snapshot(false);
    let (preview, out) = repo.json(&["migrate", "--revert"]);
    assert!(out.status.success() && preview["ok"] == true && preview["data"]["committed"] == false, "{preview}");
    assert_unchanged(&migrated, &repo, "the revert preview");
    let digest = preview["data"]["planDigest"].as_str().expect("the revert preview prints a planDigest").to_string();
    let (done, out) = repo.json(&["migrate", "--revert", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true && done["data"]["committed"] == true, "{done}");

    let after = repo.snapshot(true);
    assert_eq!(changed(&before, &after), Vec::<PathBuf>::new(), "the tree differs from before the migration");
    assert_eq!(after.get(Path::new(".canon/hot.db")), before.get(Path::new(".canon/hot.db")), "the sqlite file differs");
    // The backup the migration made is left over but ignored (git shows nothing); the index is what it was.
    assert_eq!(repo.status(), Vec::<String>::new(), "git status after the revert");
    assert!(repo.path().join(".canon/migrate-backup").is_dir(), "the revert removed the backup");
    // And the repo reads as 0.29 again.
    let (gate, _) = repo.json(&["gate", "check"]);
    assert!(violations(&gate)
        .iter()
        .any(|(class, _)| class == "old-layout"));
}

/// ledger.migrate.09 -- Then: `--backup <DIR>` names the backup to restore; a revert with no
/// backup is refused and writes nothing.
#[test]
fn ledger_migrate_09b_revert_from_a_named_backup_and_without_one() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    let (doc, out) = repo.json(&["migrate", "--revert"]);
    assert!(!out.status.success() && doc["ok"] == false, "a revert with nothing to restore is not refused: {doc}");
    assert_unchanged(&before, &repo, "the refused revert");

    let done = repo.migrate();
    let backup = done["data"]["backupDir"].as_str().expect("the commit names its backup directory").to_string();
    let (preview, out) = repo.json(&["migrate", "--revert", "--backup", &backup]);
    assert!(out.status.success() && preview["ok"] == true, "{preview}");
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let (done, out) = repo.json(&["migrate", "--revert", "--backup", &backup, "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true, "{done}");
    assert_eq!(changed(&before, &repo.snapshot(true)), Vec::<PathBuf>::new());
}

/// A 0.29 object in the local object store: the archived evidence of `auth.login.01`, named
/// the 0.29 way. Returns its key and body.
fn seed_old_object(repo: &Repo) -> (String, Value) {
    let body: Value = serde_json::from_slice(&fixture_bytes(".canon/archive/kind=evidence_record/auth.login.01__c429c5afaa0c.json")).unwrap();
    let key = "kind=evidence_record/auth.login.01__c429c5afaa0c.parquet".to_string();
    let put = ObjectRekey { from: key.clone(), to: key.clone(), kind: "evidence_record".into(), natural_key: "auth.login.01".into(), digest: "c429c5afaa0c".into(), at: body["at"].as_str().unwrap().into(), body: body.clone() };
    repo.object_store().rekey_objects(&[put]).unwrap();
    (key, body)
}

/// ledger.migrate.09 / .10 -- Then: the configured object store's keys are re-keyed through the
/// plan (its old keys recorded), and a revert restores them from the old keys the plan recorded.
#[test]
fn ledger_migrate_09c_object_store_keys_are_rekeyed_and_restored() {
    let repo = Repo::old().with_object_store();
    let (old_key, old_body) = seed_old_object(&repo);
    let before = repo.snapshot(false);

    let preview = repo.preview();
    let planned = effects_of(&preview, "rekey").into_iter().find(|e| e["record"].as_str().is_some_and(|r| r.starts_with("s3:"))).expect("an s3 re-key effect in the preview");
    assert_eq!(planned["record"], format!("s3:{old_key}").as_str());
    let new_key = planned["detail"].as_str().unwrap().strip_prefix("to ").unwrap().to_string();
    assert_eq!(new_key, "kind=evidence/root_scenario_auth.login.01_c817aa1b.parquet");

    repo.migrate();
    let objects = repo.object_store().all_objects().unwrap();
    assert_eq!(objects.len(), 1, "{objects:?}");
    assert_eq!((objects[0].key.as_str(), objects[0].kind.as_str()), (new_key.as_str(), "evidence"));
    assert_eq!(objects[0].body["kind"], "evidence");
    assert_eq!(objects[0].digest, digest8(&objects[0].body), "the object's digest is not the new body's");

    let (preview, out) = repo.json(&["migrate", "--revert"]);
    assert!(out.status.success(), "{preview}");
    assert!(effects(&preview).iter().any(|e| e["action"] == "rekey" && e["record"] == format!("s3:{new_key}").as_str() && e["detail"] == format!("back to {old_key}").as_str()), "the revert preview does not name the object's old key: {:?}", effects_of(&preview, "rekey"));
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let (done, out) = repo.json(&["migrate", "--revert", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true, "{done}");

    let objects = repo.object_store().all_objects().unwrap();
    assert_eq!(objects.len(), 1, "{objects:?}");
    assert_eq!((objects[0].key.as_str(), objects[0].kind.as_str()), (old_key.as_str(), "evidence_record"));
    assert_eq!(objects[0].body, old_body, "the object's body was not restored");
    assert_eq!(changed(&before, &repo.snapshot(true)), Vec::<PathBuf>::new(), "the tree differs from before the migration");
}

// ---------------------------------------------------------------------------
// ledger.migrate.10

/// ledger.migrate.10 -- Then: each sqlite row has its 0.30 key and its kind spelled `evidence`
/// (an 8-hex digest, the body's), the run's and the tasks' ids are the 0.30 ones.
#[test]
fn ledger_migrate_10_sqlite_rows_take_their_0_30_keys() {
    let repo = Repo::old();
    let preview = repo.preview();
    let planned: Vec<String> = effects_of(&preview, "rekey").into_iter().map(|e| format!("{} {}", e["record"].as_str().unwrap(), e["detail"].as_str().unwrap())).collect();
    repo.migrate();

    let rows = SqliteTier::connect_for_read(&repo.path().join(".canon/hot.db")).unwrap().all_rows().unwrap();
    let keys: BTreeSet<(String, String)> = rows.iter().map(|r| (r.kind.clone(), r.id.clone())).collect();
    let expected: BTreeSet<(String, String)> = [("evidence", "root_scenario_auth.login.04"), ("run", "01m5wjnysxw7zm26q8zjtf70c1"), ("task", "add-login+231"), ("task", "add-login+232")].into_iter().map(|(k, i)| (k.to_string(), i.to_string())).collect();
    assert_eq!(keys, expected, "the rows after the move; the plan said {planned:?}");
    for row in &rows {
        assert_eq!(row.digest.len(), 8, "{}:{} keeps a 12-hex digest", row.kind, row.id);
        assert_eq!(row.digest, digest8(&row.body), "{}:{}: the digest is not the body's", row.kind, row.id);
        assert!(planned.iter().any(|p| p.ends_with(&format!("to {}:{}:{}", row.kind, row.id, row.digest))), "{}:{}:{} is not the key the preview named: {planned:?}", row.kind, row.id, row.digest);
    }
    let evidence = rows.iter().find(|r| r.kind == "evidence").unwrap();
    assert_eq!(evidence.body["kind"], "evidence");
}

/// Another row takes `target` with other JSON: the `add-login#2` task row is moved there.
fn hold_key_with_other_json(repo: &Repo, target: (String, String, String)) {
    let tier = SqliteTier::connect(&repo.path().join(".canon/hot.db")).unwrap();
    let task = tier.all_rows().unwrap().into_iter().find(|r| r.kind == "task" && r.id == "add-login#2").unwrap();
    tier.rekey(&[RowRekey { from: (task.kind, task.id, task.digest), to: target, body: json!({"kind": "evidence", "held": "by another row"}) }]).unwrap();
    tier.checkpoint().unwrap();
}

/// ledger.migrate.10 -- Then: a plan's key that another row holds with other JSON refuses the
/// whole run: nothing changes (tree and sqlite file byte-identical), also for a commit with the
/// digest the earlier preview printed.
#[test]
fn ledger_migrate_10b_a_row_holding_a_planned_key_with_other_json_refuses_the_run() {
    let repo = Repo::old();
    let stale = repo.plan_digest();
    let preview = repo.preview();
    let planned = effects_of(&preview, "rekey").into_iter().find(|e| e["record"].as_str().is_some_and(|r| r.starts_with("sqlite:hot.db:evidence_record:auth.login.04:"))).expect("the evidence row's re-key");
    let target: Vec<&str> = planned["detail"].as_str().unwrap().strip_prefix("to ").unwrap().split(':').collect();
    let target = (target[0].to_string(), target[1].to_string(), target[2].to_string());
    hold_key_with_other_json(&repo, target);
    let before = repo.snapshot(false);
    let (doc, out) = repo.json(&["migrate"]);
    assert!(!out.status.success() && doc["ok"] == false, "the preview is not refused: {doc}");
    assert_unchanged(&before, &repo, "the refused preview");
    let (doc, out) = repo.json(&["migrate", "--commit", "--plan-digest", &stale]);
    assert!(!out.status.success() && doc["ok"] == false, "the commit is not refused: {doc}");
    assert_unchanged(&before, &repo, "the refused commit");
    assert!(repo.status().is_empty(), "git status: {:?}", repo.status());
}

/// ledger.migrate.10 -- Then: an object whose planned key another object holds with other JSON
/// refuses the whole run and leaves the store and the tree as they were.
#[test]
fn ledger_migrate_10c_an_object_holding_a_planned_key_with_other_json_refuses_the_run() {
    let repo = Repo::old().with_object_store();
    let (_, body) = seed_old_object(&repo);
    let target = "kind=evidence/root_scenario_auth.login.01_c817aa1b.parquet";
    let other = json!({"kind": "evidence", "held": "by another object"});
    repo.object_store()
        .rekey_objects(&[ObjectRekey { from: target.into(), to: target.into(), kind: "evidence".into(), natural_key: "root_scenario_auth.login.01".into(), digest: "c817aa1b".into(), at: body["at"].as_str().unwrap().into(), body: other }])
        .unwrap();
    let before = repo.snapshot(false);
    let objects_before: Vec<_> = repo.object_store().all_objects().unwrap().into_iter().map(|o| (o.key, o.body)).collect();

    let stale = "0".repeat(64);
    for args in [vec!["migrate"], vec!["migrate", "--commit", "--plan-digest", &stale]] {
        let (doc, out) = repo.json(&args);
        assert!(!out.status.success() && doc["ok"] == false, "`canon {args:?}` is not refused: {doc}");
    }
    assert_unchanged(&before, &repo, "the refused run");
    let objects_after: Vec<_> = repo.object_store().all_objects().unwrap().into_iter().map(|o| (o.key, o.body)).collect();
    assert_eq!(objects_after, objects_before, "the object store changed");
}

// ---------------------------------------------------------------------------
// ledger.migrate.11

/// ledger.migrate.11 -- Then: after the move, evidence recorded by `canon evidence add` lands in
/// `kind=evidence/`, the file is not ignored and `git status` lists it.
#[test]
fn ledger_migrate_11_evidence_written_after_the_move_shows_up_in_git_status() {
    let repo = Repo::old();
    repo.migrate();
    repo.commit_all("migrated");
    let (doc, out) = evidence_add(&repo);
    assert!(out.status.success() && doc["ok"] == true, "{doc}");
    let committed = doc["data"]["records"][0]["committed"].as_str().expect("the committed path").to_string();
    let name = committed.strip_prefix("kind=evidence/").unwrap_or_else(|| panic!("the record is not under kind=evidence/: {committed}"));
    let stem = name.strip_suffix(".json").unwrap();
    let (components, digest) = stem.rsplit_once('_').unwrap();
    assert_eq!(components, "root_scenario_auth.login.01", "{committed}");
    assert_eq!(digest.len(), 8, "{committed}");

    let path = format!(".canon/ledger/{committed}");
    assert!(repo.path().join(&path).is_file());
    assert_eq!(repo.git_exit(&["check-ignore", "-q", &path]), Some(1), "`git check-ignore` says {path} is ignored");
    let status = repo.status();
    assert!(status.contains(&format!("?? {path}")), "`git status --porcelain` does not list {path}:\n{}", status.join("\n"));
    let body: Value = serde_json::from_str(&repo.read(&path)).unwrap();
    assert_eq!(body["kind"], "evidence");
    assert_eq!(digest, digest8(&body));
}

// ---------------------------------------------------------------------------
// ledger.migrate.12

/// ledger.migrate.12 -- Then: a canon.yaml whose routing says `evidence_record` fails neither
/// `canon status` nor `canon context` on configuration; the key routes the kind and a warning
/// names `canon migrate`.
#[test]
fn ledger_migrate_12_the_old_key_routes_the_kind_and_warns() {
    let repo = Repo::old();
    repo.migrate();
    repo.commit_all("migrated");
    let current = repo.read("canon.yaml");
    assert!(current.contains("\n  evidence: local\n"));
    let route = |key: &str, rung: &str| repo.write("canon.yaml", &current.replacen("\n  evidence: local\n", &format!("\n  {key}: {rung}\n"), 1));

    // The new key sends evidence to `hot`: the sqlite row only. Baseline for the old key below.
    route("evidence", "hot");
    let (hot, out) = repo.json(&["query", "--kind", "evidence", "--all"]);
    assert!(out.status.success(), "{hot}");
    let on_hot = hot["data"]["count"].clone();
    route("evidence", "local");
    let on_local = repo.json(&["query", "--kind", "evidence", "--all"]).0["data"]["count"].clone();
    assert_ne!(on_hot, on_local, "the two rungs hold the same number of records; the test cannot tell them apart");

    route("evidence_record", "hot");
    for args in [["status"], ["context"]] {
        let (doc, out) = repo.json(&args);
        assert!(out.status.success() && doc["ok"] == true, "`canon {args:?}` fails with the old key: {doc}");
        assert!(doc["error"]["code"].as_str().is_none_or(|c| !c.starts_with("CONFIG")), "{doc}");
        let named = warnings(&doc).into_iter().any(|w| w.contains("canon migrate") && w.contains("evidence_record"));
        assert!(named, "`canon {args:?}` has no warning naming `evidence_record` and `canon migrate`: {:?}", warnings(&doc));
    }
    let (routed, out) = repo.json(&["query", "--kind", "evidence", "--all"]);
    assert!(out.status.success(), "{routed}");
    assert_eq!(routed["data"]["count"], on_hot, "the old `evidence_record` key does not route the kind (ledger: {on_local}, hot: {on_hot})");
}

/// ledger.migrate.12 -- Then: canon itself never writes the 0.29 key: not `canon init`, not
/// `canon migrate`, not `canon evidence add` on a migrated repo.
#[test]
fn ledger_migrate_12b_canon_never_writes_the_old_key() {
    // `canon init` in an empty repo.
    let fresh = Repo::empty();
    let (doc, out) = fresh.json(&["init"]);
    assert!(out.status.success() && doc["ok"] == true, "{doc}");
    for (path, bytes) in fresh.snapshot(false) {
        assert!(!text(&bytes).contains("evidence_record"), "`canon init` wrote `evidence_record` into {}", path.display());
    }
    assert!(fresh
        .read("canon.yaml")
        .lines()
        .any(|l| l.trim() == "evidence: local"));

    // `canon migrate` and then `canon evidence add` on the fixture.
    let repo = Repo::old();
    repo.migrate();
    repo.commit_all("migrated");
    let (add, out) = evidence_add(&repo);
    assert!(out.status.success() && add["ok"] == true, "{add}");
    for file in ["canon.yaml", ".gitignore"] {
        assert!(!repo.read(file).contains("evidence_record"), "{file} names `evidence_record` after migrate and evidence add:\n{}", repo.read(file));
    }
    let new_evidence = add["data"]["records"][0]["committed"].as_str().unwrap();
    assert!(!repo
        .read(&format!(".canon/ledger/{new_evidence}"))
        .contains("evidence_record"));
}

// ---------------------------------------------------------------------------
// ledger.migrate.14 / .15

/// ledger.migrate.14 -- Then: `canon scenario new` and `canon feature new` are each refused
/// OLD_LAYOUT with the `canon migrate` hint and write no file.
#[test]
fn ledger_migrate_14_scenario_and_feature_authoring_is_refused() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    assert_old_layout_refusal(&repo, &["scenario", "new", "auth.login.05", "--title", "A fifth login"]);
    assert_unchanged(&before, &repo, "the refused `canon scenario new`");
    assert_old_layout_refusal(&repo, &["feature", "new", "auth.signup", "--title", "Sign up"]);
    assert_unchanged(&before, &repo, "the refused `canon feature new`");
    assert!(repo.status().is_empty(), "git status: {:?}", repo.status());
}

/// ledger.migrate.15 -- Then: `canon divergence add` and `canon gate promote` are each refused
/// OLD_LAYOUT with the `canon migrate` hint and stage or commit nothing.
#[test]
fn ledger_migrate_15_divergence_writes_and_promotion_are_refused() {
    let repo = Repo::old();
    let before = repo.snapshot(false);
    assert_old_layout_refusal(&repo, &["divergence", "add", "auth.login.01", "--actor-id", "r", "--role", "reviewer"]);
    assert_unchanged(&before, &repo, "the refused `canon divergence add`");
    assert_old_layout_refusal(&repo, &["gate", "promote"]);
    assert_unchanged(&before, &repo, "the refused `canon gate promote`");
    // The staged divergence is still staged, the committed one untouched.
    assert!(repo.path().join(".canon/ledger/_staging/kind=divergence/area=auth/root__auth.login.02__0000000000__000002__f4af29d00335.json").is_file());
    assert!(repo.status().is_empty(), "git status: {:?}", repo.status());
}
