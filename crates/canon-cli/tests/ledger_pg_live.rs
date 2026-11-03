//! `canon migrate` against a REAL Postgres (scenarios `ledger.migrate.09` and `ledger.migrate.10`,
//! change `canon-0-30-ledger-naming`): a 0.29 repo whose hot tier is a Postgres schema is re-keyed
//! through the plan and restored from the old keys the plan recorded; a planned key another row holds
//! with other JSON refuses the whole run.
//!
//! Gated at run time like the store's live tests (`crates/canon-store/tests/pg_tier_live.rs`): without
//! `CANON_PG_DSN` (or with its server unreachable) each test prints a skip line and passes, and with
//! `CANON_REQUIRE_LIVE=1` that is a failure instead. CI's `live-tier` job sets both.
//!
//! Every test builds its own repo from `tests/fixtures/ledger-0.29` and its own Postgres schema (named
//! per run), so the schema holds exactly the rows the test seeded.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use canon_store::pg_tier::PgTier;
use canon_store::sqlite_tier::SqliteTier;
use canon_store::tier::{RawWrite, RowRekey, StoredRow, Tier};
use serde_json::{Value, json};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// The live gate.

fn require_live() -> bool {
    std::env::var("CANON_REQUIRE_LIVE").as_deref() == Ok("1")
}

/// `postgres://user:pass@host[:port]/db` -> `(host, port)`.
fn dsn_host_port(dsn: &str) -> (String, u16) {
    let after_scheme = dsn.split("://").nth(1).unwrap_or(dsn);
    let after_creds = after_scheme.rsplit_once('@').map_or(after_scheme, |(_, rest)| rest);
    let host_port = after_creds.split('/').next().unwrap_or(after_creds);
    match host_port.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(5432)),
        None => (host_port.to_string(), 5432),
    }
}

/// The DSN to run against, or `None` when the test is to be skipped. A missing or unreachable
/// Postgres is a hard failure under `CANON_REQUIRE_LIVE=1`.
fn live_dsn() -> Option<String> {
    let Ok(dsn) = std::env::var("CANON_PG_DSN") else {
        assert!(!require_live(), "CANON_REQUIRE_LIVE=1 but CANON_PG_DSN is unset — run `docker compose up -d --wait postgres` and set it");
        eprintln!("skipping: CANON_PG_DSN is unset (set CANON_REQUIRE_LIVE=1 to make this a hard failure)");
        return None;
    };
    let (host, port) = dsn_host_port(&dsn);
    let reachable = (host.as_str(), port).to_socket_addrs().is_ok_and(|addrs| addrs.into_iter().any(|a| TcpStream::connect_timeout(&a, Duration::from_millis(800)).is_ok()));
    if reachable {
        return Some(dsn);
    }
    assert!(!require_live(), "CANON_REQUIRE_LIVE=1 but Postgres at {host}:{port} is unreachable — run `docker compose up -d --wait postgres` first");
    eprintln!("skipping: Postgres at {host}:{port} is unreachable (set CANON_REQUIRE_LIVE=1 to make this a hard failure)");
    None
}

// ---------------------------------------------------------------------------
// The repo: the 0.29 fixture with its hot tier on a Postgres schema.

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger-0.29")
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

type Snapshot = BTreeMap<PathBuf, Vec<u8>>;

struct PgRepo {
    dir: TempDir,
    home: TempDir,
    dsn: String,
    schema: String,
    /// The Postgres rows as seeded: the 0.29 keys and bodies of the fixture's sqlite rows.
    seeded: Vec<StoredRow>,
}

impl PgRepo {
    /// The 0.29 fixture in a git repo with one commit, its `hot` tier pointed at a fresh Postgres schema
    /// that holds the fixture's sqlite rows under their 0.29 keys (the sqlite file itself is dropped).
    fn old(dsn: &str) -> Self {
        let dir = TempDir::new().unwrap();
        copy_tree(&fixture_dir(), dir.path());
        fs::rename(dir.path().join("gitignore"), dir.path().join(".gitignore")).unwrap();

        let hot_db = dir.path().join(".canon/hot.db");
        let rows = SqliteTier::connect_for_read(&hot_db).unwrap().all_rows().unwrap();
        assert!(!rows.is_empty(), "the fixture's sqlite file holds no rows");
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(dir.path().join(format!(".canon/hot.db{suffix}")));
        }

        let schema = format!("canon_ln_cli_{}", uuid::Uuid::new_v4().simple());
        let yaml = fs::read_to_string(dir.path().join("canon.yaml")).unwrap();
        let sqlite_hot = "  hot:\n    backend: sqlite\n    path: .canon/hot.db\n";
        assert!(yaml.contains(sqlite_hot), "the fixture's canon.yaml changed");
        let yaml = yaml.replacen(sqlite_hot, &format!("  hot:\n    backend: postgres\n    dsn_env: CANON_PG_DSN\n    schema: {schema}\n"), 1);
        fs::write(dir.path().join("canon.yaml"), yaml).unwrap();

        let repo = Self { dir, home: TempDir::new().unwrap(), dsn: dsn.to_string(), schema, seeded: Vec::new() };
        let seeded = repo.seed(&rows);
        let repo = Self { seeded, ..repo };
        repo.git(&["init", "-q"]);
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-q", "--allow-empty", "-m", "fixture"]);
        repo
    }

    /// Writes every row's 0.30 form through the tier, then re-keys it to the row's 0.29 key and body.
    fn seed(&self, rows: &[StoredRow]) -> Vec<StoredRow> {
        let tier = self.tier();
        for row in rows {
            let mut new_body = row.body.clone();
            if row.kind == "evidence_record" {
                new_body["kind"] = json!("evidence");
            }
            tier.write(&RawWrite(canon_model::RawRecord(new_body.clone()))).unwrap();
            let kind = canon_model::RecordKind::ALL.into_iter().find(|k| k.as_str() == new_body["kind"].as_str().unwrap()).expect("a core kind");
            let name = canon_store::partition::resolve_name(kind, &new_body).unwrap();
            let stored = tier.all_rows().unwrap().into_iter().find(|r| r.kind == kind.as_str() && r.id == name.key && r.digest == name.digest).expect("the row just written");
            tier.rekey(&[RowRekey { from: (stored.kind, stored.id, stored.digest), to: (row.kind.clone(), row.id.clone(), row.digest.clone()), body: row.body.clone() }]).unwrap();
        }
        let seeded = tier.all_rows().unwrap();
        let keys: BTreeSet<_> = seeded.iter().map(|r| (r.kind.clone(), r.id.clone(), r.digest.clone())).collect();
        let old: BTreeSet<_> = rows.iter().map(|r| (r.kind.clone(), r.id.clone(), r.digest.clone())).collect();
        assert_eq!(keys, old, "the schema holds the fixture's 0.29 keys");
        seeded
    }

    fn tier(&self) -> PgTier {
        PgTier::connect(&self.dsn, &self.schema).expect("connect to the repo's schema")
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(["-c", "gc.auto=0", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(self.path())
            .env("HOME", self.home.path())
            .env("GIT_AUTHOR_NAME", "ln-tests")
            .env("GIT_AUTHOR_EMAIL", "ln-tests@example.com")
            .env("GIT_COMMITTER_NAME", "ln-tests")
            .env("GIT_COMMITTER_EMAIL", "ln-tests@example.com")
            .output()
            .expect("spawn git");
        assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    }

    /// `canon <args> --json`: the document on stdout and the process output.
    fn json(&self, args: &[&str]) -> (Value, Output) {
        let mut all = args.to_vec();
        all.push("--json");
        let out = Command::new(env!("CARGO_BIN_EXE_canon"))
            .args(&all)
            .current_dir(self.path())
            .env_remove("CANON_ROLE")
            .env("CANON_ACTOR", "ln-tests")
            .env("CANON_PG_DSN", &self.dsn)
            .env("HOME", self.home.path())
            .output()
            .expect("spawn canon binary");
        let doc = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("`canon {all:?}` printed no JSON document ({e}):\nstdout: {}\nstderr: {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)));
        (doc, out)
    }

    /// Every file of the work tree (`.git` excluded; `.canon/migrate-backup` too unless `with_backup`).
    fn snapshot(&self, with_backup: bool) -> Snapshot {
        let mut files = Vec::new();
        files_under(self.path(), &mut files);
        files
            .into_iter()
            .map(|p| p.strip_prefix(self.path()).unwrap().to_path_buf())
            .filter(|p| !p.starts_with(".git") && (with_backup || !p.starts_with(".canon/migrate-backup")))
            .map(|p| {
                let bytes = fs::read(self.path().join(&p)).unwrap();
                (p, bytes)
            })
            .collect()
    }

    fn preview(&self) -> Value {
        let (doc, out) = self.json(&["migrate"]);
        assert!(out.status.success() && doc["ok"] == true && doc["data"]["committed"] == false, "the preview failed: {doc}");
        doc
    }

    /// The preview, then `--commit --plan-digest` with its digest.
    fn migrate(&self) -> Value {
        let digest = self.preview()["data"]["planDigest"].as_str().expect("the preview prints a planDigest").to_string();
        let (doc, out) = self.json(&["migrate", "--commit", "--plan-digest", &digest]);
        assert!(out.status.success() && doc["ok"] == true && doc["data"]["committed"] == true, "the commit failed: {doc}\n{}", String::from_utf8_lossy(&out.stderr));
        doc
    }

    fn rows(&self) -> Vec<StoredRow> {
        self.tier().all_rows().unwrap()
    }
}

fn effects_of<'a>(doc: &'a Value, action: &str) -> Vec<&'a Value> {
    doc["data"]["effects"].as_array().expect("effects").iter().filter(|e| e["action"] == action).collect()
}

fn changed(before: &Snapshot, after: &Snapshot) -> Vec<PathBuf> {
    before.keys().chain(after.keys()).collect::<BTreeSet<_>>().into_iter().filter(|p| before.get(*p) != after.get(*p)).cloned().collect()
}

fn keys_of(rows: &[StoredRow]) -> BTreeSet<(String, String)> {
    rows.iter().map(|r| (r.kind.clone(), r.id.clone())).collect()
}

// ---------------------------------------------------------------------------
// ledger.migrate.09 / .10

/// ledger.migrate.09 / .10 -- the plan re-keys the Postgres rows (kind `evidence`, 8-hex digests, the
/// 0.30 ids) and says so in the preview without touching them; reverting restores the rows' old keys
/// and bodies from the plan's record and the tree from the backup.
#[test]
fn ledger_migrate_09_postgres_rows_are_rekeyed_and_the_revert_restores_their_old_keys() {
    let Some(dsn) = live_dsn() else { return };
    let repo = PgRepo::old(&dsn);
    let tree_before = repo.snapshot(false);
    let rows_before = repo.rows();
    assert_eq!(rows_before, repo.seeded);

    // The preview plans the re-keys and writes nothing.
    let preview = repo.preview();
    let planned: Vec<String> = effects_of(&preview, "rekey")
        .into_iter()
        .filter(|e| e["record"].as_str().is_some_and(|r| r.starts_with(&format!("postgres:{}:", repo.schema))))
        .map(|e| format!("{} {}", e["record"].as_str().unwrap(), e["detail"].as_str().unwrap()))
        .collect();
    assert!(!planned.is_empty(), "no postgres re-key in the preview: {:?}", effects_of(&preview, "rekey"));
    assert_eq!(repo.rows(), rows_before, "the preview changed the rows");
    assert!(changed(&tree_before, &repo.snapshot(false)).is_empty(), "the preview changed the tree");

    // The commit: every row has its 0.30 key and its body's 8-hex digest.
    repo.migrate();
    let rows = repo.rows();
    let expected: BTreeSet<(String, String)> = [("evidence", "root_scenario_auth.login.04"), ("run", "01m5wjnysxw7zm26q8zjtf70c1"), ("task", "add-login+231"), ("task", "add-login+232")].into_iter().map(|(k, i)| (k.to_string(), i.to_string())).collect();
    assert_eq!(keys_of(&rows), expected, "the rows after the move; the plan said {planned:?}");
    for row in &rows {
        assert_eq!(row.digest.len(), 8, "{}:{} keeps a 12-hex digest", row.kind, row.id);
        assert!(planned.iter().any(|p| p.ends_with(&format!("to {}:{}:{}", row.kind, row.id, row.digest))), "{}:{}:{} is not the key the preview named: {planned:?}", row.kind, row.id, row.digest);
    }
    let evidence = rows.iter().find(|r| r.kind == "evidence").unwrap();
    assert_eq!(evidence.body["kind"], "evidence");

    // The revert preview names each row's old key; committing it restores rows and tree.
    let (preview, out) = repo.json(&["migrate", "--revert"]);
    assert!(out.status.success() && preview["ok"] == true && preview["data"]["committed"] == false, "{preview}");
    let back: Vec<&str> = effects_of(&preview, "rekey").into_iter().filter_map(|e| e["detail"].as_str()).collect();
    for old in &rows_before {
        let want = format!("back to {}:{}:{}", old.kind, old.id, old.digest);
        assert!(back.contains(&want.as_str()), "the revert preview does not say `{want}`: {back:?}");
    }
    assert_eq!(repo.rows(), rows, "the revert preview changed the rows");
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let (done, out) = repo.json(&["migrate", "--revert", "--commit", "--plan-digest", &digest]);
    assert!(out.status.success() && done["ok"] == true && done["data"]["committed"] == true, "{done}\n{}", String::from_utf8_lossy(&out.stderr));

    assert_eq!(repo.rows(), rows_before, "the rows have their 0.29 keys and bodies back");
    assert_eq!(changed(&tree_before, &repo.snapshot(false)), Vec::<PathBuf>::new(), "the tree differs from before the migration");
}

/// ledger.migrate.10 -- a Postgres row that holds a key the plan gives another row, with other JSON,
/// refuses the whole run: the preview and a commit with the digest of the earlier preview are both
/// refused, and the rows and the tree are as they were. (The planner stops on the held row, whose body
/// names nothing under the 0.30 rule; the store's own refusal of a re-key onto a held key, and its
/// rollback of the whole call, is `pg_tier_live.rs`'s `ledger_migrate_10_a_rekey_onto_a_held_key_...`.)
#[test]
fn ledger_migrate_10_a_postgres_row_holding_a_planned_key_with_other_json_refuses_the_run() {
    let Some(dsn) = live_dsn() else { return };
    let repo = PgRepo::old(&dsn);
    let preview = repo.preview();
    let stale = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let prefix = format!("postgres:{}:evidence_record:auth.login.04:", repo.schema);
    let planned = effects_of(&preview, "rekey").into_iter().find(|e| e["record"].as_str().is_some_and(|r| r.starts_with(&prefix))).expect("the evidence row's re-key");
    let target: Vec<&str> = planned["detail"].as_str().unwrap().strip_prefix("to ").unwrap().split(':').collect();
    let target = (target[0].to_string(), target[1].to_string(), target[2].to_string());

    // Another row takes that key with other JSON: the `add-login#2` task row is moved there.
    let tier = repo.tier();
    let task = tier.all_rows().unwrap().into_iter().find(|r| r.kind == "task" && r.id == "add-login#2").unwrap();
    tier.rekey(&[RowRekey { from: (task.kind, task.id, task.digest), to: target, body: json!({"kind": "evidence", "held": "by another row"}) }]).unwrap();
    drop(tier);

    let rows_before = repo.rows();
    let tree_before = repo.snapshot(true);
    let (doc, out) = repo.json(&["migrate"]);
    assert!(!out.status.success() && doc["ok"] == false, "the preview is not refused: {doc}");
    assert_eq!(doc["error"]["code"], "MIGRATE_REFUSED", "{doc}");
    let (doc, out) = repo.json(&["migrate", "--commit", "--plan-digest", &stale]);
    assert!(!out.status.success() && doc["ok"] == false, "the commit is not refused: {doc}");
    assert_eq!(repo.rows(), rows_before, "the rows changed");
    assert_eq!(changed(&tree_before, &repo.snapshot(true)), Vec::<PathBuf>::new(), "the tree changed");
}
