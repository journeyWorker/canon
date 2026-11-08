//! `canon-0-39-session-kinds-removal`: the record kinds `session` and `event` are gone (twelve kinds remain) and the rows an older
//! canon stored stay readable only to `canon purge`, through the real binary.
//!
//! F1 an old `canon.yaml` that routes or ages them loads, warns `RETIRED_KIND_KEYS` and never refuses a write, F2 `canon migrate` deletes
//! exactly those lines (and `--revert` restores them), F3 `canon purge --kind session|event` deletes the stored rows in every tier whatever
//! the routing says, F4 `query`, `export` and `search` refuse the two names, F7 a `kind=session/` directory in the ledger breaks nothing,
//! F8 `canon init` routes the twelve kinds and no retired one.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::envelope::RecordKind;
use serde_json::Value;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// The repo and what it is run with.

/// A git work tree and the `HOME` every spawn gets.
struct Repo {
    dir: TempDir,
    home: TempDir,
}

/// The retired routing keys an old scaffold carried, inserted after `run`'s line.
const OLD_ROUTING: &str = "  session: hot\n  event: hot\n";
/// The retired aging key, and the section it leaves empty once migrated.
const OLD_AGING: &str = "aging:\n  event: { after: 7d, to: cold }\n";

/// A `canon.yaml` of `canon init` with the retired keys an older canon wrote added.
fn with_retired_keys(fresh: &str) -> String {
    let marker = "  run: hot\n";
    assert!(fresh.contains(marker), "init no longer routes `run` to hot:\n{fresh}");
    format!("{}{OLD_AGING}", fresh.replacen(marker, &format!("{marker}{OLD_ROUTING}"), 1))
}

/// The inverse of [`with_retired_keys`]: the text before the keys were added.
fn without_retired_keys(old: &str) -> String {
    old.replacen(OLD_ROUTING, "", 1).replacen(OLD_AGING, "", 1)
}

impl Repo {
    fn bare() -> Self {
        let repo = Self { dir: TempDir::new().unwrap(), home: TempDir::new().unwrap() };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    /// A `canon init` repo with one commit.
    fn init() -> Self {
        let repo = Self::bare();
        let out = repo.run(&["init", "--no-agents-md", "--policy", "light", "--no-harness"]);
        assert!(out.status.success(), "canon init: {}", combined(&out));
        repo.commit_all("init");
        repo
    }

    /// A `canon init` repo whose `canon.yaml` is the one an older canon wrote: `routing.session`, `routing.event` and `aging.event`.
    fn old() -> Self {
        let repo = Self::init();
        let fresh = repo.read("canon.yaml");
        repo.write("canon.yaml", &with_retired_keys(&fresh));
        repo.commit_all("an old canon.yaml");
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "gc.auto=0", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(self.path())
            .env("HOME", self.home.path())
            .env("GIT_AUTHOR_NAME", "kinds-tests")
            .env("GIT_AUTHOR_EMAIL", "kinds-tests@example.com")
            .env("GIT_COMMITTER_NAME", "kinds-tests")
            .env("GIT_COMMITTER_EMAIL", "kinds-tests@example.com")
            .output()
            .expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
    }

    fn run(&self, args: &[&str]) -> Output {
        hermetic::canon().args(args).current_dir(self.path()).env("HOME", self.home.path()).env("CANON_ACTOR", "kinds-tests").output().expect("spawn canon binary")
    }

    /// `canon <args> --json`: the document and the process output.
    fn json(&self, args: &[&str]) -> (Value, Output) {
        let mut all = args.to_vec();
        all.push("--json");
        let out = self.run(&all);
        let doc = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("`canon {all:?}` printed no JSON document ({e}):\nstdout: {}\nstderr: {}", text(&out.stdout), text(&out.stderr)));
        (doc, out)
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path().join(relative)).unwrap_or_else(|e| panic!("`{relative}`: {e}"))
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn exists(&self, relative: &str) -> bool {
        self.path().join(relative).exists()
    }

    /// Every file under the repo but `.git` and the SQLite sidecars, with its bytes.
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if name == ".git" || name.ends_with("-wal") || name.ends_with("-shm") {
                    continue;
                }
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    out.insert(path.strip_prefix(root).unwrap().to_path_buf(), fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(self.path(), self.path(), &mut out);
        out
    }

    /// The `planDigest` of the preview `canon <args>` prints; the preview must succeed and commit nothing.
    fn preview_digest(&self, args: &[&str]) -> String {
        let (doc, out) = self.json(args);
        assert!(out.status.success() && doc["ok"] == true, "`canon {args:?}` preview failed: {doc}");
        assert_eq!(doc["data"]["committed"], false, "a bare run must not commit: {doc}");
        doc["data"]["planDigest"].as_str().unwrap_or_else(|| panic!("the preview prints a planDigest: {doc}")).to_string()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn combined(out: &Output) -> String {
    format!("{}{}", text(&out.stdout), text(&out.stderr))
}

fn warning<'a>(doc: &'a Value, code: &str) -> Option<&'a Value> {
    doc["meta"]["warnings"].as_array()?.iter().find(|w| w["code"] == code)
}

fn error_code(doc: &Value) -> &str {
    doc["error"]["code"].as_str().unwrap_or_default()
}

/// The 1-based number of the line of `text` that is exactly `line`.
fn line_of(text: &str, line: &str) -> usize {
    text.lines().position(|l| l == line).unwrap_or_else(|| panic!("no line `{line}` in:\n{text}")) + 1
}

/// The twelve kinds, as `--kind` spells them.
fn kinds() -> Vec<String> {
    RecordKind::ALL.iter().map(|kind| kind.as_str().to_string()).collect()
}

/// `canon <args>` read the repo: it succeeded, or, for `gate check` on a repo with no spec corpus, said it had examined nothing (`UNVERIFIABLE`), with no
/// violation and no config error.
fn assert_reads(args: &[&str], doc: &Value, out: &Output, what: &str) {
    if args == ["gate", "check"] {
        assert_eq!(doc["data"]["violations"], serde_json::json!([]), "`canon gate check` with {what}: {doc}");
        assert!(doc["error"].is_null() || error_code(doc) == "UNVERIFIABLE", "`canon gate check` with {what}: {doc}");
        assert!(out.status.success() || error_code(doc) == "UNVERIFIABLE", "`canon gate check` with {what}: {doc}");
    } else {
        assert!(out.status.success() && doc["ok"] == true, "`canon {args:?}` with {what}: {doc}");
    }
}

const RETIRED_KEYS: [&str; 3] = ["routing.session", "routing.event", "aging.event"];

// ---------------------------------------------------------------------------
// F1: an old canon.yaml loads and warns.

/// tier.retired.01 (F1):
/// F1: `status`, `gate check` and `context` read a `canon.yaml` that still routes and ages the retired kinds: they exit without an error, warn
/// `RETIRED_KIND_KEYS` naming each key, and lead `meta.next` with `canon migrate --json`.
#[test]
fn tier_retired_01_old_retired_keys_load_and_warn_with_the_migrate_step_first() {
    let repo = Repo::old();
    for args in [&["status"][..], &["gate", "check"], &["context"]] {
        let (doc, out) = repo.json(args);
        assert_reads(args, &doc, &out, "the retired keys");
        let warn = warning(&doc, "RETIRED_KIND_KEYS").unwrap_or_else(|| panic!("`canon {args:?}` does not warn RETIRED_KIND_KEYS: {doc}"));
        let message = warn["message"].as_str().unwrap();
        for key in RETIRED_KEYS {
            assert!(message.contains(&format!("`{key}`")), "{args:?}: the warning does not name `{key}`: {message}");
        }
        assert_eq!(doc["meta"]["next"][0]["command"], "canon migrate --json", "{args:?}: {doc}");
    }
}

/// tier.retired.01 (F1):
/// F1: the keys' values are never read: a value that is no rung, in either section, loads the same.
#[test]
fn tier_retired_01_the_values_of_the_retired_keys_are_not_checked() {
    let repo = Repo::old();
    let odd = repo.read("canon.yaml").replace("  session: hot\n", "  session: not-a-rung\n").replace("event: { after: 7d, to: cold }", "event: { after: soon, to: nowhere }");
    repo.write("canon.yaml", &odd);
    let (doc, out) = repo.json(&["status"]);
    assert!(out.status.success(), "{doc}");
    assert!(warning(&doc, "RETIRED_KIND_KEYS").is_some(), "{doc}");
}

/// tier.retired.02 (F1, failure path):
/// F1: the retired names are the only unknown ones read as keys that route nothing; any other misspelled kind is still a config error.
#[test]
fn tier_retired_02_a_misspelled_kind_in_routing_is_still_a_config_error() {
    let repo = Repo::old();
    let typo = repo.read("canon.yaml").replace("  session: hot\n", "  sesion: hot\n");
    repo.write("canon.yaml", &typo);
    let (doc, out) = repo.json(&["status"]);
    assert!(!out.status.success(), "`routing.sesion` loaded: {doc}");
    assert_eq!(doc["ok"], false, "{doc}");
    assert!(doc.to_string().contains("sesion"), "the refusal names the key: {doc}");
}

/// tier.retired.03 (F1, failure path):
/// F1: unlike the 0.29 layout, the keys never refuse a write: a repo with them takes `change new` and `evidence add`.
#[test]
fn tier_retired_03_writes_are_not_refused_in_a_repo_with_the_retired_keys() {
    let repo = Repo::init();
    for args in [
        &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"][..],
        &["feature", "new", "auth.login", "--title", "Login"],
        &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"],
    ] {
        let out = repo.run(args);
        assert!(out.status.success(), "setup `canon {args:?}`: {}", combined(&out));
    }
    repo.commit_all("setup");
    repo.write("canon.yaml", &with_retired_keys(&repo.read("canon.yaml")));
    repo.commit_all("an old canon.yaml");

    let (change, out) = repo.json(&["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    assert_ne!(error_code(&change), "LAYOUT_MIGRATION_REQUIRED", "{change}");
    assert!(out.status.success() && change["ok"] == true, "`change new` in a repo with retired keys: {change}");
    let (evidence, out) = repo.json(&["evidence", "add", "--scenario", "auth.login.01", "--project", "root", "--class", "test-run", "--ref", "cargo test", "--role", "implementer"]);
    assert_ne!(error_code(&evidence), "LAYOUT_MIGRATION_REQUIRED", "{evidence}");
    assert!(out.status.success() && evidence["ok"] == true, "`evidence add` in a repo with retired keys: {evidence}");
}

// ---------------------------------------------------------------------------
// F2: `canon migrate` deletes the keys.

/// tier.retired.04 (F2):
/// F2: the preview names the lines it deletes by number and writes nothing; the commit deletes exactly those lines; the warning is gone and
/// the next plan is empty; `--revert` restores the old file byte for byte.
#[test]
fn tier_retired_04_migrate_deletes_exactly_the_retired_key_lines_and_revert_restores_them() {
    let repo = Repo::old();
    let old = repo.read("canon.yaml");
    let expected = format!("{}aging: {{}}\n", without_retired_keys(&old));
    let before = repo.snapshot();

    let (preview, out) = repo.json(&["migrate"]);
    assert!(out.status.success() && preview["ok"] == true && preview["data"]["committed"] == false, "{preview}");
    assert_eq!(repo.snapshot(), before, "the preview wrote something");
    let shown = preview["data"]["effects"].to_string();
    for (key, line) in [("routing.session", "  session: hot"), ("routing.event", "  event: hot"), ("aging.event", "  event: { after: 7d, to: cold }")] {
        assert!(shown.contains(&format!("line {} (`{key}`)", line_of(&old, line))), "the preview does not name {key} by its line number: {shown}");
    }
    let human = combined(&repo.run(&["migrate"]));
    assert!(human.contains("delete line"), "the human preview names the lines: {human}");

    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let (done, out) = repo.json(&["migrate", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true && done["data"]["committed"] == true, "{done}");
    let migrated = repo.read("canon.yaml");
    assert_eq!(migrated, expected, "the commit changed more or less than the retired lines");
    assert!(migrated.contains("aging: {}\n"), "a section left without a key is `{{}}`: {migrated}");

    let (status, out) = repo.json(&["status"]);
    assert!(out.status.success(), "{status}");
    assert!(warning(&status, "RETIRED_KIND_KEYS").is_none(), "the warning survives the migration: {status}");
    let (again, out) = repo.json(&["migrate"]);
    assert!(out.status.success() && again["ok"] == true, "{again}");
    assert!(again["data"]["effects"].as_array().is_none_or(Vec::is_empty), "a second plan still has effects: {again}");
    assert!(combined(&repo.run(&["migrate"])).contains("nothing to migrate"), "a second run has nothing to migrate");

    let revert = repo.preview_digest(&["migrate", "--revert"]);
    let (reverted, out) = repo.json(&["migrate", "--revert", "--commit", "--plan-digest", &revert]);
    assert!(out.status.success() && reverted["ok"] == true && reverted["data"]["committed"] == true, "{reverted}");
    assert_eq!(repo.read("canon.yaml"), old, "the revert did not restore canon.yaml");
    let (status, _) = repo.json(&["status"]);
    assert!(warning(&status, "RETIRED_KIND_KEYS").is_some(), "the restored file warns again: {status}");
}

/// tier.retired.05 (F2, failure path):
/// F2: a key that cannot be deleted line by line (a flow mapping, an anchor) is refused as a config error and the file stays as it was.
#[test]
fn tier_retired_05_a_flow_mapping_or_an_anchored_key_is_refused_and_the_file_is_unchanged() {
    for config in [
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting: { change: local, session: hot }\n",
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n  session: &shared hot\n  task: *shared\n",
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n  session: !!str hot\n",
    ] {
        let repo = Repo::bare();
        repo.write("canon.yaml", config);
        repo.commit_all("config");
        let before = repo.snapshot();
        for args in [&["migrate"][..], &["migrate", "--commit", "--plan-digest", "sha256:0000000000000000000000000000000000000000000000000000000000000000"]] {
            let (doc, out) = repo.json(args);
            assert_eq!(out.status.code(), Some(2), "{config}: {doc}");
            assert_eq!(error_code(&doc), "CONFIG_INVALID", "{config}: {doc}");
            assert!(doc["error"]["message"].as_str().unwrap().contains("by hand"), "the refusal tells the hand edit: {doc}");
        }
        assert_eq!(repo.snapshot(), before, "a refused migrate changed the tree for {config}");
    }
}

/// tier.retired.06 (F2, failure path):
/// F2: a file under `kind=session/` named the 0.29 way (a `__<digest12>` stem) is not a file `canon migrate` renames or refuses, even when it
/// is no JSON at all.
#[test]
fn tier_retired_06_a_0_29_named_file_under_kind_session_does_not_make_migrate_refuse() {
    let repo = Repo::old();
    repo.write(".canon/ledger/kind=session/legacy-session__0123456789ab.json", "this is not json");
    repo.write(".canon/ledger/kind=session/area=x/another__abcdef012345.json", "{\"kind\":\"session\"}");
    repo.commit_all("0.29-named rows of a retired kind");
    let before = repo.snapshot();

    let (preview, out) = repo.json(&["migrate"]);
    assert!(out.status.success() && preview["ok"] == true, "migrate refused a retired kind's file: {preview}");
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    assert!(!preview["data"]["effects"].to_string().contains("kind=session"), "the plan moves a retired kind's file: {preview}");
    let (done, out) = repo.json(&["migrate", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true, "{done}");
    let after = repo.snapshot();
    for (path, bytes) in &before {
        if path.starts_with(".canon/ledger/kind=session") {
            assert_eq!(after.get(path), Some(bytes), "{} changed", path.display());
        }
    }
    assert!(!repo.read("canon.yaml").contains("session: hot"), "the keys were not deleted");
}

// ---------------------------------------------------------------------------
// F3: `canon purge --kind session|event` deletes the stored rows.

/// The `records_history` row of a retired kind: what the sqlite hot tier of an older canon stored.
fn hot_row(repo: &Repo, kind: &str, id: &str, at: &str) {
    let path = repo.path().join(".canon/hot.db");
    // The tier makes the table; the row is then written as an older canon wrote it.
    drop(canon_store::sqlite_tier::SqliteTier::connect(&path).unwrap());
    let db = rusqlite::Connection::open(&path).unwrap();
    let body = serde_json::json!({"kind": kind, "at": at, "id": id}).to_string();
    db.execute("INSERT INTO records_history (kind, id, at, digest, body) VALUES (?1, ?2, ?3, ?4, ?5)", rusqlite::params![kind, id, at, "d0000001", body]).unwrap();
}

fn hot_ids(repo: &Repo, kind: &str) -> Vec<String> {
    let db = rusqlite::Connection::open(repo.path().join(".canon/hot.db")).unwrap();
    let mut statement = db.prepare("SELECT id FROM records_history WHERE kind = ?1 ORDER BY id").unwrap();
    let rows = statement.query_map([kind], |row| row.get::<_, String>(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

/// A hand-written git-tier row of a retired kind: a JSON file whose body carries an `at`.
fn git_row(repo: &Repo, kind: &str, name: &str, at: &str) {
    repo.write(&format!(".canon/ledger/kind={kind}/{name}.json"), &serde_json::json!({"kind": kind, "at": at, "note": "stored by an older canon"}).to_string());
}

fn plant_rows(repo: &Repo) {
    git_row(repo, "session", "old-session", "2026-01-01T00:00:00Z");
    git_row(repo, "session", "new-session", "2027-06-01T00:00:00Z");
    git_row(repo, "event", "old-event", "2026-01-01T00:00:00Z");
    hot_row(repo, "session", "old-hot-session", "2026-01-01T00:00:00+00:00");
    hot_row(repo, "session", "new-hot-session", "2027-06-01T00:00:00+00:00");
    hot_row(repo, "event", "old-hot-event", "2026-01-01T00:00:00+00:00");
    repo.commit_all("rows of the retired kinds");
}

fn matched(doc: &Value) -> u64 {
    doc["data"]["rungs"].as_array().unwrap().iter().map(|rung| rung["matched"].as_u64().unwrap()).sum()
}

fn deleted(doc: &Value) -> u64 {
    doc["data"]["rungs"].as_array().unwrap().iter().map(|rung| rung["deleted"].as_u64().unwrap()).sum()
}

fn delete_effects(doc: &Value) -> usize {
    doc["data"]["effects"].as_array().unwrap().iter().filter(|effect| effect["action"] == "delete").count()
}

/// The purge of the retired `session` kind, the whole flow: the preview matches the one old row of each of the git and the sqlite tier and writes
/// nothing, a stale digest is refused, the commit deletes only those rows (the `event` rows and the newer `session` rows stay).
fn purge_session_flow(repo: &Repo) {
    plant_rows(repo);
    let before = repo.snapshot();

    let (preview, out) = repo.json(&["purge", "--kind", "session", "--before", "2026-06-01"]);
    assert!(out.status.success() && preview["ok"] == true && preview["data"]["committed"] == false, "{preview}");
    assert_eq!(matched(&preview), 2, "one old session row in the git tier and one in the sqlite file: {preview}");
    assert_eq!(delete_effects(&preview), 2, "{preview}");
    assert_eq!(preview["data"]["kind"], "session");
    assert_eq!(repo.snapshot(), before, "the preview deleted something");
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();

    // A removal set that is another set than the one previewed is refused, and nothing is deleted.
    git_row(repo, "session", "second-old-session", "2026-02-01T00:00:00Z");
    let (stale, out) = repo.json(&["purge", "--kind", "session", "--before", "2026-06-01", "--commit", "--plan-digest", &digest]);
    assert_eq!(out.status.code(), Some(2), "{stale}");
    assert_eq!(error_code(&stale), "PLAN_STALE", "{stale}");
    assert!(repo.exists(".canon/ledger/kind=session/old-session.json") && repo.exists(".canon/ledger/kind=session/second-old-session.json"), "a stale digest deleted a row");
    fs::remove_file(repo.path().join(".canon/ledger/kind=session/second-old-session.json")).unwrap();
    assert_eq!(repo.snapshot(), before, "the refusal and the cleanup left the tree as planted");

    let (done, out) = repo.json(&["purge", "--kind", "session", "--before", "2026-06-01", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true && done["data"]["committed"] == true, "{done}");
    assert_eq!((matched(&done), deleted(&done)), (2, 2), "{done}");
    assert!(!repo.exists(".canon/ledger/kind=session/old-session.json"), "the old git row is still there");
    assert!(repo.exists(".canon/ledger/kind=session/new-session.json"), "the newer git row was deleted");
    assert!(repo.exists(".canon/ledger/kind=event/old-event.json"), "the purge of `session` deleted an `event` row");
    assert_eq!(hot_ids(repo, "session"), ["new-hot-session"], "only the old sqlite row goes");
    assert_eq!(hot_ids(repo, "event"), ["old-hot-event"], "the purge of `session` deleted an `event` row of the sqlite file");

    // The rows of `event` go the same way, and only when asked for.
    let digest = repo.preview_digest(&["purge", "--kind", "event", "--before", "2026-06-01"]);
    let (done, out) = repo.json(&["purge", "--kind", "event", "--before", "2026-06-01", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && deleted(&done) == 2, "{done}");
    assert!(!repo.exists(".canon/ledger/kind=event/old-event.json"));
    assert!(hot_ids(repo, "event").is_empty());
    assert_eq!(hot_ids(repo, "session"), ["new-hot-session"]);
}

/// tier.retired.07 (F3):
/// F3: with `routing.session` and `routing.event` still in `canon.yaml`.
#[test]
fn tier_retired_07_purge_session_deletes_the_stored_rows_of_every_tier() {
    purge_session_flow(&Repo::old());
}

/// tier.retired.07 (F3):
/// F3: the same after `canon migrate` deleted the keys: a retired kind is scanned in every tier `canon.yaml` configures, routed or not.
#[test]
fn tier_retired_07_purge_session_finds_the_rows_after_the_routing_keys_were_migrated_away() {
    let repo = Repo::old();
    let digest = repo.preview_digest(&["migrate"]);
    let (done, out) = repo.json(&["migrate", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true, "{done}");
    assert!(!repo.read("canon.yaml").contains("session: hot"));
    purge_session_flow(&repo);
}

/// tier.retired.08 (F3, failure path):
/// F3: only the four names are purgeable; every other kind keeps its closed-value refusal, and the rows stay.
#[test]
fn tier_retired_08_purge_of_another_kind_is_refused_and_deletes_nothing() {
    let repo = Repo::old();
    plant_rows(&repo);
    let before = repo.snapshot();
    for kind in ["change", "task", "evidence"] {
        let out = repo.run(&["purge", "--kind", kind, "--before", "2026-06-01"]);
        assert_eq!(out.status.code(), Some(2), "{kind}: {}", combined(&out));
        assert!(combined(&out).contains("invalid value") && combined(&out).contains("expected: run, trajectory, session, event"), "{kind}: {}", combined(&out));
    }
    // The same refusal for a kind named in an `--input` document lists the same four values.
    let doc = serde_json::json!({"kind": "change", "before": "2026-06-01T00:00:00Z"}).to_string();
    let (refused, out) = repo.json(&["purge", "--input", &doc]);
    assert_eq!(out.status.code(), Some(2), "{refused}");
    assert_eq!(error_code(&refused), "INPUT_INVALID", "{refused}");
    assert_eq!(refused["error"]["expected"], serde_json::json!(["run", "trajectory", "session", "event"]), "{refused}");
    assert_eq!(repo.snapshot(), before, "a refused purge changed the tree");
    assert_eq!(hot_ids(&repo, "session").len(), 2);
}

// ---------------------------------------------------------------------------
// F4: the readers refuse the two names.

/// tier.retired.09 (F4):
/// F4: `query`, `export` and `search` refuse `--kind session` and `--kind event` as a usage error that lists the twelve kinds and points at `canon purge`.
#[test]
fn tier_retired_09_the_readers_refuse_the_retired_kind_names() {
    let repo = Repo::init();
    let twelve = kinds();
    assert_eq!(twelve.len(), 12);
    assert!(!twelve.iter().any(|kind| kind == "session" || kind == "event"));
    for (args, name) in [
        (&["query", "--kind", "session"][..], "session"),
        (&["query", "--kind", "event"], "event"),
        (&["export", "--kind", "event"], "event"),
        (&["export", "--kind", "session"], "session"),
        (&["search", "x", "--kind", "session"], "session"),
        (&["search", "x", "--kind", "event"], "event"),
    ] {
        let (doc, out) = repo.json(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {doc}");
        assert_eq!(doc["ok"], false, "{args:?}: {doc}");
        assert_eq!(error_code(&doc), "USAGE", "{args:?}: {doc}");
        assert_eq!(doc["error"]["field"], "--kind", "{args:?}: {doc}");
        let expected: Vec<String> = doc["error"]["expected"].as_array().unwrap_or_else(|| panic!("{args:?}: {doc}")).iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(expected, twelve, "{args:?}: {doc}");
        let message = doc["error"]["message"].as_str().unwrap();
        assert!(message.contains(&format!("--kind {name} was removed in 0.39.0")) && message.contains("`session` and `event` record kinds no longer exist"), "{args:?}: {message}");
        assert!(doc["error"]["hint"].as_str().unwrap().contains(&format!("canon purge --kind {name} --before <date>")), "{args:?}: {doc}");
    }
}

/// tier.retired.10 (F4, failure path):
/// F4: the twelve kinds still read, and `purge` still takes the retired names.
#[test]
fn tier_retired_10_the_twelve_kinds_still_read() {
    let repo = Repo::init();
    for kind in ["run", "evidence"] {
        let (doc, out) = repo.json(&["query", "--kind", kind]);
        assert!(out.status.success() && doc["ok"] == true, "query --kind {kind}: {doc}");
    }
    let (doc, out) = repo.json(&["search", "x", "--kind", "run"]);
    assert!(out.status.success() && doc["ok"] == true, "search --kind run: {doc}");
    let (doc, out) = repo.json(&["purge", "--kind", "session", "--before", "2020-01-01"]);
    assert!(out.status.success() && doc["ok"] == true, "purge --kind session: {doc}");
}

// ---------------------------------------------------------------------------
// F7: a `kind=session/` directory in the ledger.

/// tier.retired.15 (F7):
/// F7: rows of a retired kind sit in the git tier, whatever their body (even none that parses): `status`, `gate check` and `ledger compact` succeed and
/// leave the files as they were.
#[test]
fn tier_retired_15_a_kind_session_directory_in_the_ledger_breaks_no_command_and_is_left_alone() {
    let repo = Repo::init();
    repo.write(".canon/ledger/kind=session/parseable.json", "{\"kind\":\"session\",\"at\":\"2026-01-01T00:00:00Z\"}");
    repo.write(".canon/ledger/kind=session/garbage.json", "this is not json {");
    repo.write(".canon/ledger/kind=session/legacy__0123456789ab.json", "");
    repo.write(".canon/ledger/kind=event/area=x/old.json", "[1, 2");
    repo.commit_all("rows of the retired kinds");
    let planted: BTreeMap<PathBuf, Vec<u8>> = repo.snapshot().into_iter().filter(|(path, _)| path.starts_with(".canon/ledger/kind=session") || path.starts_with(".canon/ledger/kind=event")).collect();
    assert_eq!(planted.len(), 4);

    for args in [&["status"][..], &["gate", "check"]] {
        let (doc, out) = repo.json(args);
        assert_reads(args, &doc, &out, "a kind=session/ directory");
        assert!(!doc.to_string().contains("kind=session"), "`canon {args:?}` names a retired kind's file: {doc}");
    }
    let digest = repo.preview_digest(&["ledger", "compact"]);
    let (done, out) = repo.json(&["ledger", "compact", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true, "`ledger compact` with a kind=session/ directory: {done}");

    let after = repo.snapshot();
    for (path, bytes) in &planted {
        assert_eq!(after.get(path), Some(bytes), "{} was touched", path.display());
    }
}

// ---------------------------------------------------------------------------
// F8: what `canon init` writes.

/// The `routing:` section of `canon.yaml`: kind to rung.
fn routing(text: &str) -> BTreeMap<String, String> {
    let doc: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
    doc["routing"].as_mapping().unwrap().iter().map(|(k, v)| (k.as_str().unwrap().to_string(), v.as_str().unwrap().to_string())).collect()
}

/// tier.retired.16 (F8):
/// F8: a fresh scaffold routes the twelve kinds and no retired one, `task`, `handoff` and `run` to hot, and `init --check` passes without a warning.
#[test]
fn tier_retired_16_init_routes_the_twelve_kinds_and_no_retired_one() {
    let repo = Repo::init();
    let text = repo.read("canon.yaml");
    for retired in ["session", "event"] {
        assert!(!text.lines().any(|line| line.trim_start().starts_with(&format!("{retired}:"))), "init wrote a `{retired}` line:\n{text}");
    }
    assert!(!text.contains("session/run/event"), "the header still names the retired kinds:\n{text}");
    let routes = routing(&text);
    assert_eq!(routes.keys().cloned().collect::<BTreeSet<_>>(), kinds().into_iter().collect::<BTreeSet<_>>(), "every kind is routed, and only the twelve");
    let hot: BTreeSet<&str> = routes.iter().filter(|(_, rung)| *rung == "hot").map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(hot, BTreeSet::from(["task", "handoff", "run"]), "{routes:?}");
    assert!(routes.iter().filter(|(kind, _)| !hot.contains(kind.as_str())).all(|(_, rung)| rung == "local"), "{routes:?}");

    let (check, out) = repo.json(&["init", "--check"]);
    assert!(out.status.success() && check["ok"] == true, "init --check on a fresh scaffold: {check}");
    assert!(warning(&check, "RETIRED_KIND_KEYS").is_none(), "{check}");
}

/// tier.retired.17 (F8, failure path):
/// F8: an old scaffold (routing lines for the retired kinds) gets the F1 warning in `init --check`, which still passes.
#[test]
fn tier_retired_17_init_check_warns_of_an_old_scaffold() {
    let repo = Repo::old();
    let (check, out) = repo.json(&["init", "--check"]);
    assert!(out.status.success() && check["ok"] == true, "init --check on an old scaffold: {check}");
    let warn = warning(&check, "RETIRED_KIND_KEYS").unwrap_or_else(|| panic!("init --check does not warn: {check}"));
    for key in RETIRED_KEYS {
        assert!(warn["message"].as_str().unwrap().contains(&format!("`{key}`")), "{key}: {warn}");
    }
    assert_eq!(check["meta"]["next"][0]["command"], "canon migrate --json", "{check}");
}

/// tier.retired.11 (F5): `canon context` lists the twelve kinds and the model's schema directory holds no `session` or `event` schema.
#[test]
fn tier_retired_11_context_and_the_schema_files_have_twelve_kinds_and_no_retired_one() {
    let repo = Repo::init();
    let (doc, out) = repo.json(&["context"]);
    assert!(out.status.success(), "{doc}");
    let listed: BTreeSet<String> = doc["data"]["kinds"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(listed, kinds().into_iter().collect::<BTreeSet<_>>(), "{doc}");
    assert_eq!(listed.len(), 12);
    assert!(!listed.contains("session") && !listed.contains("event"), "{listed:?}");
    let schemas = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../canon-model/schemas");
    for retired in ["session", "event"] {
        assert!(!schemas.join(format!("{retired}.schema.json")).exists(), "{retired}.schema.json is still shipped");
    }
}

fn repo_text(relative: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative)).unwrap()
}

/// tier.retired.18 (F9, failure path): the 0.39 section of the CHANGELOG names the retired keys and the warning, the one-release purge, the refusals and the removed marts;
/// a missing entry fails this release check naming it.
#[test]
fn tier_retired_18_the_changelog_lists_the_retired_kinds_and_what_replaces_them() {
    let changelog = repo_text("CHANGELOG.md");
    let unreleased = changelog.split("\n## ").find(|section| section.starts_with("Unreleased") || section.starts_with("0.39")).unwrap_or_default();
    for needed in ["`RETIRED_KIND_KEYS`", "`routing.session`", "`routing.event`", "`aging.event`", "`canon migrate`", "`canon purge --kind session|event`", "`--kind session|event`", "`mart_session_costs`", "`## Session costs`"] {
        assert!(unreleased.contains(needed), "the 0.39 section of CHANGELOG.md does not mention {needed}");
    }
}

/// tier.retired.19 (F9): the architecture document, the README and the website list no record kind named `session` or `event`.
#[test]
fn tier_retired_19_the_guides_do_not_list_the_retired_kinds() {
    let mut files = vec!["README.md".to_string(), "ARCHITECTURE.md".to_string()];
    for dir in ["packages/website/src/content/docs", "packages/website/src/content/docs/ko", "packages/website/src/content/docs/concepts", "packages/website/src/content/docs/ko/concepts"] {
        for entry in fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "mdx") {
                files.push(format!("{dir}/{}", path.file_name().unwrap().to_string_lossy()));
            }
        }
    }
    for file in files {
        let text = repo_text(&file);
        for listing in ["`session`, `event`", "`session` and `event`", "session|event", "session, event", "`event`, `session`"] {
            assert!(!text.contains(listing), "{file} lists the retired kinds as `{listing}`");
        }
    }
}

/// What `canon <args>` says that a retired directory could change: the exit status, the error code, the violations and the warning codes.
fn verdict_of(repo: &Repo, args: &[&str]) -> (Option<i32>, String, Value, Vec<String>) {
    let (doc, out) = repo.json(args);
    let warnings = doc["meta"]["warnings"].as_array().into_iter().flatten().map(|w| w["code"].as_str().unwrap_or_default().to_string()).collect();
    (out.status.code(), error_code(&doc).to_string(), doc["data"]["violations"].clone(), warnings)
}

/// tier.retired.20 (F7, failure path):
/// F7: files of `kind=session/` and `kind=event/` whose bodies cannot be read (not UTF-8, empty, truncated, the wrong shape) add no violation, warning or error to `status`,
/// `gate check`, `inventory sync` and `ledger compact`: each says what it says without them, and the files are as they were. The same truncated body under a live kind is named.
#[test]
fn tier_retired_20_unreadable_bodies_in_the_retired_directories_change_nothing_and_under_a_live_kind_are_named() {
    let repo = Repo::init();
    let commands: [&[&str]; 4] = [&["status"], &["gate", "check"], &["inventory", "sync"], &["ledger", "compact"]];
    let before: Vec<_> = commands.iter().map(|args| verdict_of(&repo, args)).collect();

    let bodies: [(&str, &[u8]); 4] = [("binary", b"\xff\xfe\x00\x80"), ("empty", b""), ("truncated", b"{\"kind\":\"session\",\"at\":"), ("array", b"[1, 2")];
    for (kind, dir) in [("session", "kind=session"), ("event", "kind=event/area=x")] {
        for (name, body) in bodies {
            let path = repo.path().join(format!(".canon/ledger/{dir}/{kind}-{name}.json"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        }
    }
    repo.commit_all("unreadable rows of the retired kinds");
    let planted: BTreeMap<PathBuf, Vec<u8>> = repo.snapshot().into_iter().filter(|(path, _)| path.starts_with(".canon/ledger/kind=session") || path.starts_with(".canon/ledger/kind=event")).collect();
    assert_eq!(planted.len(), 8);

    for (args, was) in commands.iter().zip(&before) {
        let (doc, _) = repo.json(args);
        assert_eq!(&verdict_of(&repo, args), was, "`canon {args:?}` says something else with unreadable rows of the retired kinds");
        let said = doc.to_string();
        assert!(!said.contains("kind=session") && !said.contains("kind=event"), "`canon {args:?}` names a retired directory: {doc}");
    }
    let after = repo.snapshot();
    for (path, bytes) in &planted {
        assert_eq!(after.get(path), Some(bytes), "{} was touched", path.display());
    }

    // The same truncated body under a live kind is not ignored: the reading above is not a reader that reads nothing.
    repo.write(".canon/ledger/kind=evidence/truncated.json", "{\"kind\":\"evidence\",\"at\":");
    let (doc, out) = repo.json(&["gate", "check"]);
    assert_eq!(out.status.code(), Some(1), "{doc}");
    let named: Vec<String> = doc["data"]["violations"].as_array().unwrap().iter().map(|v| v["subject"].as_str().unwrap_or_default().to_string()).collect();
    assert!(named.iter().any(|subject| subject.contains("kind=evidence")) && !named.iter().any(|subject| subject.contains("kind=session") || subject.contains("kind=event")), "{named:?}");
}
